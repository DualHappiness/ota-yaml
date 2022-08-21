use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::connection::EventType;

enum PushType {
    None,
    Slient,
    Force,
}
impl std::fmt::Display for PushType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PushType::None => write!(f, "none"),
            PushType::Slient => write!(f, "slient"),
            PushType::Force => write!(f, "force"),
        }
    }
}
pub struct Carside {
    auto_publish: bool,
    push_type: PushType,
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
            vehicle.id,
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

    pub async fn process(&self, ota: &super::Ota, vehicle: &super::Vehicle) -> Result<()> {
        if self.auto_publish {
            self.publish(ota, vehicle).await?;
        }
        Ok(())
    }
}
