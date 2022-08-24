use std::{collections::HashMap, io::Write};

use anyhow::Result;
use colored::*;
use connection::{Connenction, EventType};
use serde::{Deserialize, Serialize};

mod auth;
mod carside;
mod connection;

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Vehicle {
    id: i32,
    name: String,
}
pub struct Ota {
    user_id: i32,
    conn: Connenction,
    vehicles: Vec<Vehicle>,
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

        let req = RequestBody {
            user_id: self.user_id,
            station_id: "".to_string(),
            name: "".to_string(),
        };
        let resp: ResponseBody = self
            .conn
            .request(
                EventType::OtaFetchVehicleTemplateTable,
                "?pageSize=10000",
                &req,
            )
            .await?;
        tracing::info!(
            "get {} vehicles from ota, total {}",
            resp.list.len(),
            resp.total
        );
        self.vehicles = Ota::get_vehicles(resp.list.as_slice())?;
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
        let req = RequsetBody {
            vehicle_id: vehicle.id,
        };
        let str: serde_json::Value = self
            .conn
            .request(EventType::OtaFetchVehicleTemplateItemContents, "", &req)
            .await?;
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
        let req = RequsetBody {
            user_id: self.user_id,
            vehicle_id: vehicle.id,
            old_config: serde_yaml::to_string(old)?,
            new_config: serde_yaml::to_string(new)?,
        };
        let resp: ResponseBody = self
            .conn
            .request(EventType::OtaAddVehicleTemplateItem, "", &req)
            .await?;
        if resp.ok {
            tracing::info!("save {} success", vehicle.name);
        } else {
            tracing::error!("save {} failed: {:?}", vehicle.name, resp.message);
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
            .prompt_skippable()?
            .unwrap_or(false);
        Ok(confirm)
    }

    fn get_manual() -> Result<bool> {
        inquire::Confirm::new("manual")
            .with_help_message("need to confirm all edit mannually")
            .with_default(true)
            .prompt()
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn process(&mut self) -> Result<()> {
        let manual = Ota::get_manual()?;
        let mut carside = carside::Carside::new()?;

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
            if !manual || Ota::preview_confirm(&old, &new)? {
                self.save(&old, &new, v).await?;
                carside.process(self, v).await?;
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

    pub async fn run() -> Result<()> {
        let host = Ota::get_host()?;
        let port = 8090;
        let path = "/user_client";
        let mut ota = Ota {
            user_id: -1,
            conn: Connenction::new(&host, port, path).await?,
            vehicles: vec![],
        };

        ota.user_id = auth::auth(&ota).await?;
        ota.select_vehicle().await?;
        ota.process().await?;

        Ok(())
    }
}
