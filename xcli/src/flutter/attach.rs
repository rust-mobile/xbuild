use anyhow::Result;
use jsonrpsee::core::client::{ClientT, Subscription, SubscriptionClientT};
use jsonrpsee::ws_client::{WsClient, WsClientBuilder};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct View {
    pub id: String,
    pub isolate: Isolate,
}
#[derive(Debug, Deserialize)]
struct Isolate {
    pub id: String,
}
#[derive(Debug, Deserialize)]
struct SimpleValue {
    pub value: String,
}

#[derive(Debug, Deserialize)]
struct StreamNotify {
    pub event: RawEvent,
}

#[derive(Debug, Deserialize)]
struct RawEvent {
    #[serde(rename = "extensionData")]
    pub extension_data: Option<ExtensionData>,
}

#[derive(Debug, Deserialize)]
struct ExtensionData {
    pub extension: Option<String>,
    pub value: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Event {
    VmServiceUrl(String),
    DevToolsAddress(String),
}

pub struct VmService {
    client: WsClient,
    root_dir: PathBuf,
    target: PathBuf,
    events: Subscription<StreamNotify>,
}

impl VmService {
    pub async fn attach(url: &str, root_dir: PathBuf, target: PathBuf) -> Result<Self> {
        let resp = reqwest::blocking::get(format!("{}ws", url))?;
        let url = format!(
            "ws://{}",
            resp.url().as_str().strip_prefix("http://").unwrap()
        );
        let client = WsClientBuilder::default().build(&url).await?;
        let events: Subscription<StreamNotify> = client.subscribe_to_method("streamNotify").await?;
        let mut args = BTreeMap::new();
        args.insert("streamId", "Isolate".into());
        client
            .request::<Value>("streamListen", Some(args.into()))
            .await?;
        let mut args = BTreeMap::new();
        args.insert("streamId", "Extension".into());
        client
            .request::<Value>("streamListen", Some(args.into()))
            .await?;
        Ok(Self {
            client,
            root_dir,
            target,
            events,
        })
    }

    pub async fn get_version(&self) -> Result<(u8, u8)> {
        #[derive(Deserialize)]
        struct Version {
            major: u8,
            minor: u8,
        }
        let resp: Version = self.client.request("getVersion", None).await?;
        Ok((resp.major, resp.minor))
    }

    async fn list_views(&self) -> Result<Vec<View>> {
        #[derive(Deserialize)]
        struct Views {
            views: Vec<View>,
        }
        let views: Views = self.client.request("_flutter.listViews", None).await?;
        Ok(views.views)
    }

    async fn run_in_view(
        &self,
        view: &str,
        main_script: &Path,
        asset_directory: &Path,
    ) -> Result<()> {
        let mut args = BTreeMap::new();
        args.insert("viewId", view.to_string().into());
        args.insert(
            "mainScript",
            main_script.to_str().unwrap().to_string().into(),
        );
        args.insert(
            "assetDirectory",
            asset_directory.to_str().unwrap().to_string().into(),
        );
        self.client
            .request("_flutter.runInView", Some(args.into()))
            .await?;
        Ok(())
    }

    pub async fn reassemble(&self) -> Result<()> {
        self.client.request("ext.flutter.reassemble", None).await?;
        Ok(())
    }

    pub async fn vmservice_uri(&self) -> Result<String> {
        let value: SimpleValue = self
            .client
            .request("ext.flutter.connectedVmServiceUri", None)
            .await?;
        Ok(value.value)
    }

    pub async fn flutter_devtools_uri(&self) -> Result<Vec<String>> {
        let mut devtools = vec![];
        for view in self.list_views().await? {
            let mut args = BTreeMap::new();
            args.insert("isolateId", view.isolate.id.into());
            let value: SimpleValue = self
                .client
                .request("ext.flutter.activeDevToolsServerAddress", Some(args.into()))
                .await?;
            devtools.push(value.value);
        }
        Ok(devtools)
    }

    pub async fn hot_reload(&self) -> Result<()> {
        for view in self.list_views().await? {
            let mut args = BTreeMap::new();
            args.insert("isolateId", view.isolate.id.into());
            self.client
                .request::<Value>("reloadSources", Some(args.into()))
                .await?;
        }
        Ok(())
    }

    pub async fn hot_restart(&self) -> Result<()> {
        for view in self.list_views().await? {
            self.run_in_view(&view.id, &self.target, &self.root_dir.join("assets"))
                .await?;
        }
        Ok(())
    }

    pub async fn quit(&self) -> Result<()> {
        for view in self.list_views().await? {
            let mut args = BTreeMap::new();
            args.insert("isolateId", view.isolate.id.into());
            self.client
                .request::<Value>("ext.flutter.exit", Some(args.into()))
                .await
                .ok();
        }
        Ok(())
    }

    pub async fn next_event(&mut self) -> Result<Event> {
        loop {
            while let Some(event) = self.events.next().await {
                if let Some(ExtensionData {
                    extension: Some(ext),
                    value: Some(value),
                }) = event?.event.extension_data
                {
                    let event = match ext.as_str() {
                        "ext.flutter.connectedVmServiceUri" => Event::VmServiceUrl(value),
                        "ext.flutter.activeDevToolsServerAddress" => Event::DevToolsAddress(value),
                        _ => continue,
                    };
                    return Ok(event);
                }
            }
        }
    }
}
