use std::{collections::HashMap, io::Write};

use anyhow::Result;
use serde::{Deserialize, Serialize};
mod connection;
use colored::*;
use connection::{Connenction, EventType, Request};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Vehicle {
    id: i32,
    name: String,
}
pub struct Ota {
    user_id: i32,
    manual: bool,
    conn: Connenction,
    vehicles: Vec<Vehicle>,
}

impl Ota {
    fn get_username() -> Result<String> {
        inquire::Text::new("username")
            .prompt()
            .map_err(|e| anyhow::anyhow!(e))
    }
    fn get_password() -> Result<String> {
        inquire::Password::new("password")
            .with_display_toggle_enabled()
            .with_display_mode(inquire::PasswordDisplayMode::Masked)
            .prompt()
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn auth(&mut self) -> Result<()> {
        tracing::info!("start auth... ");
        #[derive(Serialize, Deserialize, Debug)]
        #[serde(rename_all = "camelCase")]
        struct AuthReqBody {
            username: String,
            password: String,
            organization_id: i32,
        }
        #[derive(Debug, Deserialize, Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Role {
            name: String,
        }
        #[derive(Serialize, Deserialize, Debug)]
        #[serde(rename_all = "camelCase")]
        struct AuthRespBody {
            id: i32,
            role_list: Vec<Role>,
        }
        let req_body = AuthReqBody {
            username: Ota::get_username()?,
            password: Ota::get_password()?,
            organization_id: 1,
        };
        let req = Request {
            event_type: EventType::LoginRequest,
            request_type: EventType::LoginRequest,
            path_parameter: "".to_string(),
            request_body: serde_json::to_string(&req_body)?,
        };
        tracing::debug!("auth with req: {:?}", req);
        self.conn.send(&req).await?;
        let resp_body: AuthRespBody =
            serde_json::from_value(self.conn.recv().await.map_err(|e| {
                tracing::error!("auth failed, please check usename and password.");
                anyhow::anyhow!("auth error: {:?}", e)
            })?)?;
        tracing::debug!("auth resp body: {:?}", resp_body);
        self.user_id = resp_body.id;
        Ok(())
    }
}

impl Ota {
    fn get_vehicles(vehicles: &[Vehicle]) -> Result<Vec<Vehicle>> {
        let vehicle_names: Vec<String> = vehicles.iter().map(|v| v.name.clone()).collect();
        let validator: inquire::validator::MultiOptionValidator<String> = &|v| {
            if v.len() == 0 {
                Err(String::from("no vehicle selected"))
            } else {
                Ok(())
            }
        };
        let map: HashMap<_, _> = vehicle_names.iter().cloned().zip(vehicles.iter()).collect();
        let selected = inquire::MultiSelect::new("vehicles", vehicle_names)
            .with_validator(validator)
            .prompt()?;
        tracing::debug!("vehicles: {:?}, size: {}", selected, selected.len());
        Ok(selected.iter().map(|name| map[name].clone()).collect())
    }

    async fn select_vehicle(&mut self) -> Result<()> {
        #[derive(Serialize, Deserialize, Debug)]
        #[serde(rename_all = "camelCase")]
        struct RequestBody {
            user_id: i32,
            station_id: String,
            name: String,
        }
        #[derive(Serialize, Deserialize, Debug)]
        #[serde(rename_all = "camelCase")]
        struct ResponseBody {
            total: i32,
            list: Vec<Vehicle>,
        }

        let req_body = RequestBody {
            user_id: self.user_id,
            station_id: "".to_string(),
            name: "".to_string(),
        };
        let req = Request {
            event_type: EventType::OtaFetchVehicleTemplateTable,
            request_type: EventType::OtaFetchVehicleTemplateTable,
            path_parameter: "?pageSize=10000".to_string(),
            request_body: serde_json::to_string(&req_body)?,
        };
        tracing::debug!("select vehicle with req: {:?}", req);
        self.conn.send(&req).await?;

        let resp_body: ResponseBody = serde_json::from_value(self.conn.recv().await?)?;
        tracing::info!(
            "get {} vehicles from ota, total {}",
            resp_body.list.len(),
            resp_body.total
        );
        self.vehicles = Ota::get_vehicles(resp_body.list.as_slice())?;
        Ok(())
    }
}

mod mode;
impl Ota {
    async fn get_yaml(&self, vehicle: &Vehicle) -> Result<serde_yaml::Value> {
        #[derive(Debug, Deserialize, Serialize)]
        #[serde(rename_all = "camelCase")]
        struct RequsetBody {
            vehicle_id: i32,
        }
        let req_body = RequsetBody {
            vehicle_id: vehicle.id,
        };
        let req = Request {
            event_type: EventType::OtaFetchVehicleTemplateItemContents,
            request_type: EventType::OtaFetchVehicleTemplateItemContents,
            path_parameter: "".to_string(),
            request_body: serde_json::to_string(&req_body)?,
        };
        tracing::debug!("get yaml with req: {:?}", req);
        self.conn.send(&req).await?;
        let str = self.conn.recv().await?;
        assert!(str.is_string());
        let yaml: serde_yaml::Value = serde_yaml::from_str(str.as_str().unwrap_or("")).unwrap_or(
            serde_yaml::Value::String("this is an empty yaml.".to_string()),
        );
        tracing::trace!("get {} yaml: {:?}", vehicle.name, yaml);
        Ok(yaml)
    }

    async fn save(
        &self,
        old: &serde_yaml::Value,
        new: &serde_yaml::Value,
        vehicle: &Vehicle,
    ) -> Result<()> {
        #[derive(Debug, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RequsetBody {
            user_id: i32,
            vehicle_id: i32,
            old_config: String,
            new_config: String,
        }
        #[derive(Debug, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ResponseBody {
            ok: bool,
            message: Option<String>,
        }
        let req_body = RequsetBody {
            user_id: self.user_id,
            vehicle_id: vehicle.id,
            old_config: serde_yaml::to_string(old)?,
            new_config: serde_yaml::to_string(new)?,
        };
        let req = Request {
            event_type: EventType::OtaAddVehicleTemplateItem,
            request_type: EventType::OtaAddVehicleTemplateItem,
            path_parameter: "".to_string(),
            request_body: serde_json::to_string(&req_body)?,
        };
        self.conn.send(&req).await?;
        let resp_body: ResponseBody = serde_json::from_value(self.conn.recv().await?)?;
        if resp_body.ok {
            tracing::info!("save {} success", vehicle.name);
        } else {
            tracing::error!("save {} failed: {:?}", vehicle.name, resp_body.message);
        }
        Ok(())
    }

    fn preview_confirm(old: &serde_yaml::Value, new: &serde_yaml::Value) -> Result<bool> {
        let mut file = tempfile::Builder::new()
            .prefix("temp-preview")
            .suffix(".yaml")
            .tempfile()?;
        let path = file.path().to_owned();
        file.write_all(serde_yaml::to_string(old)?.as_bytes())?;
        file.flush()?;

        inquire::Editor::new("preview")
            .with_help_message("[use :qa to quit]")
            .with_editor_command(std::ffi::OsStr::new("vimdiff"))
            .with_file_extension(".yaml")
            .with_predefined_text(serde_yaml::to_string(new)?.as_str())
            .with_args(&[
                std::ffi::OsStr::new("-c"),
                std::ffi::OsStr::new("set readonly wrap"),
                path.to_path_buf().as_os_str(),
            ])
            .prompt()?;
        let confirm = inquire::Confirm::new("confirm")
            .with_default(true)
            .prompt()?;
        Ok(confirm)
    }

    async fn publish(&self, vehicle: &Vehicle) -> Result<i32> {
        #[derive(Debug, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RequsetBody {
            modify_user_id: i32,
            vehicle_id: i32,
            bucket_name: String,
            key: String,
            name: String,
            for_test: i32,
        }
        #[derive(Debug, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ResponseBody {
            id: i32,
            vehicle_id: i32,
        }
        let now = chrono::Local::now();
        let name = format!(
            "{}-{}{}-auto.tar.gz",
            vehicle.id,
            now.format("%Y%m%d%T"),
            self.user_id
        );

        let req_body = RequsetBody {
            modify_user_id: self.user_id,
            vehicle_id: vehicle.id,
            for_test: 1,
            bucket_name: "zelos-config".to_string(),
            key: name.clone(),
            name,
        };
        let req = Request {
            event_type: EventType::OtaAddConfigurePublish,
            request_type: EventType::OtaAddConfigurePublish,
            path_parameter: "".to_string(),
            request_body: serde_json::to_string(&req_body)?,
        };
        tracing::debug!("publish with req: {:?}", req);
        self.conn.send(&req).await?;
        let resp_body: ResponseBody = serde_json::from_value(self.conn.recv().await?)?;
        tracing::debug!("publish with resp: {:?}", resp_body);
        Ok(resp_body.id)
    }

    async fn process(&mut self) -> Result<()> {
        let auto_publish = inquire::Confirm::new("auto publish")
            .with_default(true)
            .prompt()?;

        let mut modified = vec![];
        let mut skipped = vec![];
        let mut handle_map = HashMap::new();

        for v in &self.vehicles {
            tracing::info!("start process {}.", v.name);
            let old = self.get_yaml(v).await?;
            let mut new = old.clone();
            while let Some(mode) = mode::get_handle_mode()? {
                let handle = handle_map.entry(mode).or_insert(mode::get_handle(&mode));
                new = handle.handle(self, &v, &new)?;
            }
            let confirm = !self.manual || Ota::preview_confirm(&old, &new)?;
            if confirm {
                self.save(&old, &new, v).await?;
                if auto_publish {
                    let _id = self.publish(v).await?;
                    // TODO(dualwu): add confirm & push to vehicle
                }
                modified.push(v.name.clone());
            } else {
                tracing::warn!("skip {}", v.name);
                skipped.push(v.name.clone());
            }
        }
        tracing::info!("process done.");
        tracing::info!(
            r#"
summary:
modified: [{}]
skipped: [{}]
"#,
            modified.join(", ").color("green"),
            skipped.join(", ").color("red"),
        );
        Ok(())
    }
}

impl Ota {
    fn get_host() -> Result<String> {
        inquire::Text::new("ota host")
            .with_default("ota.zelostech.com.cn")
            .prompt()
            .map_err(|e| anyhow::anyhow!(e))
    }

    fn get_confirm() -> Result<bool> {
        inquire::Confirm::new("manual")
            .with_help_message("need to confirm all edit mannually")
            .with_default(true)
            .prompt()
            .map_err(|e| anyhow::anyhow!(e))
    }

    pub async fn run() -> Result<()> {
        let host = Ota::get_host()?;
        let port = 8090;
        let path = "/user_client";
        let mut ota = Ota {
            user_id: -1,
            manual: Ota::get_confirm()?,
            conn: Connenction::new(&host, port, path).await?,
            vehicles: vec![],
        };

        ota.auth().await?;
        ota.select_vehicle().await?;
        ota.process().await?;

        Ok(())
    }
}
