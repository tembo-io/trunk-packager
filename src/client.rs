use crate::Result;

use std::sync::Arc;

use bytes::Bytes;
use serde::Deserialize;

#[derive(Clone)]
pub struct Client {
    client: reqwest::Client,
    base_url: Arc<str>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Extension {
    pub name: String,
    pub license: String,
    pub latest_version: String,
    pub description: String,
}

impl Client {
    pub fn new(base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into(),
        }
    }

    /// Get the name of all currently available extensions
    pub async fn fetch_extensions(&self) -> Result<Vec<Extension>> {
        let url = format!("{}/extensions/all", self.base_url);

        self.client
            .get(url)
            .send()
            .await?
            .json()
            .await
            .map_err(Into::into)
    }

    pub async fn download_file(&self, url: &str) -> Result<Bytes> {
        let response = self.client.get(url).send().await?;

        let content = response.bytes().await?;

        Ok(content)
    }

    pub async fn fetch_extension_archive(&self, extension: &str) -> Result<Bytes> {
        let archive_url = {
            let url = format!("{}/extensions/{}/latest/download", self.base_url, extension);

            self.client.get(url).send().await?.text().await?
        };

        self.download_file(&archive_url).await
    }
}
