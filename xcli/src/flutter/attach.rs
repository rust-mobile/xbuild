use anyhow::Result;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::ws_client::{WsClient, WsClientBuilder};
use serde::Deserialize;

pub struct VmService {
    client: WsClient,
}

impl VmService {
    pub async fn attach(url: &str) -> Result<Self> {
        let url = format!("ws://{}", url.strip_prefix("http://").unwrap());
        let client = WsClientBuilder::default().build(&url).await?;
        Ok(Self { client })
    }

    pub async fn get_version(&self) -> Result<(u8, u8)> {
        let resp: Version = self.client.request("getVersion", None).await?;
        Ok((resp.major, resp.minor))
    }
}

#[derive(Deserialize)]
struct Version {
    major: u8,
    minor: u8,
}
