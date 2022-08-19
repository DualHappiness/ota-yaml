use anyhow::Result;
use futures::{StreamExt, TryStreamExt};
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

#[derive(Debug, Clone)]
pub struct Connenction {
    tx: UnboundedSender<Message>,
    rx: Arc<RwLock<UnboundedReceiver<Message>>>,
}

impl Connenction {
    pub async fn new(host: &str, port: i32, path: &str) -> Result<Connenction> {
        let url = format!("ws://{}:{}{}", host, port, path);
        tracing::info!("connent to {}", url);
        let (stream, _) = connect_async(url).await?;

        let (wtx, wrx) = futures_channel::mpsc::unbounded::<Message>();
        let (rtx, rrx) = futures_channel::mpsc::unbounded::<Message>();
        let conn = Connenction {
            tx: wtx.clone(),
            rx: Arc::new(RwLock::new(rrx)),
        };

        let (write, read) = stream.split();

        let t = wrx.map(Ok).forward(write);
        let r = read.try_for_each(move |msg| {
            tracing::trace!("recv: {:?}", msg);
            rtx.unbounded_send(msg).unwrap();
            futures_util::future::ok(())
        });

        tokio::spawn(async move {
            futures_util::pin_mut!(t, r);
            futures_util::future::select(t, r).await;
        });
        let c = conn.clone();
        tokio::spawn(async move {
            c.keep_alive().await.expect("keep alive failed");
        });
        tracing::info!("websocket connected.");
        Ok(conn)
    }

    async fn keep_alive(&self) -> Result<()> {
        let req = Request {
            event_type: EventType::KeepAlive,
            request_type: EventType::KeepAlive,
            path_parameter: "".to_string(),
            request_body: "".to_string(),
        };
        loop {
            tracing::trace!("keep alive: {:?}", req);
            self.send(&req).await?;
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventType {
    TokenLoginRequest,
    LoginRequest,
    KeepAlive,
    OtaFetchVehicleTemplateTable,
    OtaFetchVehicleTemplateItemContents,
    OtaAddVehicleTemplateItem,
    OtaAddConfigurePublish,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub event_type: EventType,
    pub request_type: EventType,
    pub path_parameter: String,
    pub request_body: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Response {
    event_type: EventType,
    error_code: i32,
    message: Option<String>,
    data: Option<serde_json::Value>,
}

impl Connenction {
    pub async fn send(&self, req: &Request) -> Result<()> {
        let msg = Message::Text(serde_json::to_string(req)?);
        self.tx.unbounded_send(msg)?;
        Ok(())
    }

    pub async fn recv(&self) -> Result<serde_json::Value> {
        let mut rx = self.rx.write().await;
        let resp: Response = serde_json::from_str(
            &rx.next()
                .await
                .ok_or(anyhow::anyhow!("no message"))?
                .to_string(),
        )?;
        tracing::debug!("recv: {:?}", resp);
        if resp.error_code != 0 {
            tracing::error!("error: {:?}", resp);
            return Err(anyhow::anyhow!(resp
                .message
                .unwrap_or("no error message".to_string())));
        }
        if resp.data.is_none() {
            tracing::error!("no data recv: {:?}", resp);
            return Err(anyhow::anyhow!("no data"));
        }
        Ok(resp.data.unwrap())
    }
}
