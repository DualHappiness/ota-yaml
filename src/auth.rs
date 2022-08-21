use crate::connection::EventType;
use crate::Ota;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct UserToken {
    token: String,
    expires_at: chrono::DateTime<chrono::Local>,
}
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

async fn auth_with_password(ota: &Ota, username: &str, password: &str) -> Result<(i32, String)> {
    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct AuthReqBody<'a> {
        username: &'a str,
        password: &'a str,
        organization_id: i32,
    }
    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct AuthRespBody {
        id: i32,
        token: String,
    }
    let req = AuthReqBody {
        username,
        password,
        organization_id: 1,
    };
    let resp: AuthRespBody = ota
        .conn
        .request(EventType::LoginRequest, "", &req)
        .await
        .map_err(|e| {
            tracing::error!("auth failed, please check username and password");
            anyhow::anyhow!("auth error: {:?}", e)
        })?;
    Ok((resp.id, resp.token))
}

async fn auth_with_token(ota: &Ota, token: &str) -> Result<i32> {
    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct AuthReqBody<'a> {
        token: &'a str,
    }
    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct AuthRespBody {
        id: i32,
    }
    let req = AuthReqBody { token };
    let resp: AuthRespBody = ota
        .conn
        .request(EventType::TokenLoginRequest, "", &req)
        .await?;
    Ok(resp.id)
}

pub async fn auth(ota: &Ota) -> Result<i32> {
    tracing::info!("start auth... ");

    let user_dir = directories::UserDirs::new().ok_or(anyhow::anyhow!("can't find home dir"))?;
    let token_file = user_dir.home_dir().join(".cache/ota-yaml/token");
    if tokio::fs::canonicalize(&token_file).await.is_ok() {
        let user_token: UserToken =
            serde_json::from_str(&tokio::fs::read_to_string(&token_file).await?)?;
        if user_token.expires_at > chrono::Local::now() {
            let id = auth_with_token(ota, &user_token.token).await?;
            return Ok(id);
        }
    }

    let username = get_username()?;
    let password = get_password()?;
    let (id, token) = auth_with_password(ota, &username, &password).await?;
    tokio::fs::create_dir_all(token_file.parent().unwrap()).await?;
    let expire_time = chrono::Duration::minutes(15);
    let user_token = UserToken {
        token: token,
        expires_at: chrono::Local::now() + expire_time,
    };
    tokio::fs::write(token_file, serde_json::to_string(&user_token)?).await?;
    tracing::info!("auth success");
    Ok(id)
}
