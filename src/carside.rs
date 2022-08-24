use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::connection::EventType;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone, Copy)]
enum PushType {
    None,
    #[serde(rename = "UPGRADE_SELIENT")]
    Slient,
    #[serde(rename = "UPGRADE_ENFORCE")]
    Force,
}
impl std::fmt::Display for PushType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PushType::None => write!(f, "not push"),
            PushType::Slient => write!(f, "slient"),
            PushType::Force => write!(f, "force"),
        }
    }
}
pub struct Carside {
    auto_publish: bool,
    push_type: PushType,
    can_approve: bool,
}

impl Carside {
    fn get_auto_publish() -> Result<bool> {
        Ok(inquire::Confirm::new("auto publish")
            .with_default(true)
            .prompt_skippable()?
            .unwrap_or(false))
    }

    pub fn new() -> anyhow::Result<Self> {
        let mut carside = Carside {
            auto_publish: Carside::get_auto_publish()?,
            push_type: PushType::None,
            can_approve: true,
        };

        if carside.auto_publish {
            carside.push_type = inquire::Select::new(
                "auto push to car",
                vec![PushType::None, PushType::Slient, PushType::Force],
            )
            .prompt_skippable()?
            .unwrap_or(PushType::None);
        }
        Ok(carside)
    }
}
impl Carside {
    async fn publish(&self, ota: &super::Ota, vehicle: &super::Vehicle) -> Result<i32> {
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
            vehicle.name,
            now.format("%Y%m%d%T"),
            ota.user_id
        );

        let req_body = RequsetBody {
            modify_user_id: ota.user_id,
            vehicle_id: vehicle.id,
            for_test: 1,
            bucket_name: "zelos-config".to_string(),
            key: name.clone(),
            name,
        };
        let resp: ResponseBody = ota
            .conn
            .request(EventType::OtaAddConfigurePublish, "", &req_body)
            .await?;
        Ok(resp.id)
    }

    async fn approve(&self, ota: &super::Ota, conf_id: i32) -> Result<bool> {
        #[derive(Debug, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ReqBody {
            id: i32,
            is_approve: bool,
            approver_id: i32,
        }
        #[derive(Debug, Serialize, Deserialize)]
        struct ResponseBody {
            ok: bool,
        }
        let req = ReqBody {
            id: conf_id,
            is_approve: true,
            approver_id: ota.user_id,
        };

        let resp: ResponseBody = ota
            .conn
            .request(EventType::OtaEditConfigurePublish, "", &req)
            .await?;
        Ok(resp.ok)
    }

    async fn push(&self, ota: &super::Ota, vehicle: &super::Vehicle, conf_id: i32) -> Result<bool> {
        #[derive(Debug, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ReqBody {
            vehicle_id: i32,
            modify_user_id: i32,
            vehicle_zelos_configure_build_history_id: i32,
            command_type: PushType,
        }
        #[derive(Debug, Serialize, Deserialize)]
        struct ResponseBody {
            ok: bool,
        }
        let req = ReqBody {
            vehicle_id: vehicle.id,
            modify_user_id: ota.user_id,
            vehicle_zelos_configure_build_history_id: conf_id,
            command_type: self.push_type,
        };
        let resp: ResponseBody = ota
            .conn
            .request(EventType::OtaConfigurePublish, "", &req)
            .await?;
        Ok(resp.ok)
    }

    pub async fn process(&mut self, ota: &super::Ota, vehicle: &super::Vehicle) -> Result<()> {
        if self.auto_publish {
            let conf_id = self.publish(ota, vehicle).await?;
            self.can_approve = self.can_approve && self.approve(ota, conf_id).await?;
            if self.push_type != PushType::None {
                if !self.can_approve {
                    tracing::warn!(
                        "skip push {} to {}, because can not approve",
                        conf_id,
                        vehicle.id
                    );
                } else if self.push(ota, vehicle, conf_id).await? {
                    tracing::info!("success push {} to {}", conf_id, vehicle.id);
                } else {
                    tracing::error!("push conf {} to {} faild.", conf_id, vehicle.id);
                }
            }
        }
        Ok(())
    }
}
