use crate::Result;

use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};

use once_cell::sync::Lazy;
use serde::Deserialize;
use tempfile::TempDir;

static BASE_URL: Lazy<String> = Lazy::new(|| std::env::var("BASE_URL").unwrap());

pub struct Client {
    client: reqwest::Client,
    dir: TempDir,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Extension {
    pub name: String,
    pub latest_version: String,
}

impl Client {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("Failed to build temporary directory");
        Self {
            client: reqwest::Client::new(),
            dir,
        }
    }

    pub fn temp_dir(&self) -> &Path {
        self.dir.path()
    }

    /// Get the name of all currently available extensions
    pub async fn fetch_extensions(&self) -> Result<Vec<Extension>> {
        let url = format!("{}/extensions/all", &*BASE_URL);

        self.client
            .get(url)
            .send()
            .await?
            .json()
            .await
            .map_err(Into::into)
    }

    pub async fn download_file(&self, url: &str) -> Result<PathBuf> {
        let response = self.client.get(url).send().await?;

        let file_name = response
            .url()
            .path_segments()
            .and_then(|segments| segments.last())
            .and_then(|name| if name.is_empty() { None } else { Some(name) })
            .expect("URL must end with a file name");

        let destination = self.dir.path().join(file_name);
        let mut file = File::create(&destination)?;

        let content = response.bytes().await?;
        io::copy(&mut content.as_ref(), &mut file)?;

        Ok(destination)
    }

    pub async fn fetch_extension_archive(&self, extension: &Extension) -> Result<PathBuf> {
        let archive_url = {
            let url = format!(
                "{}/extensions/{}/{}/download",
                &*BASE_URL, extension.name, extension.latest_version
            );

            self.client.get(url).send().await?.text().await?
        };

        self.download_file(&archive_url).await
    }
}
