mod cli;
mod client;
mod deb_packager;
mod dependencies;
mod unarchiver;
mod utils;

use std::ops::Not;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Ok};
use cli::{PackageAll, PackageOne};
use client::Extension;
use dependencies::FetchData;
use once_cell::sync::Lazy;
use owo_colors::OwoColorize;
use tempfile::TempDir;

use crate::cli::Subcommands;
use crate::client::Client;
use crate::deb_packager::DebPackager;
use crate::dependencies::Dependencies;

pub type Result<T = ()> = anyhow::Result<T>;

pub static TEMP_DIR: Lazy<TempDir> = Lazy::new(|| tempfile::tempdir().unwrap());

pub fn split_newlines(text: &str) -> impl Iterator<Item = &'_ Path> {
    text.split('\n')
        .filter(|line| line.is_empty().not())
        .map(Path::new)
}

async fn package_extension(
    base_url: String,
    trunk_project_name: String,
    export_dir: PathBuf,
    maybe_file: Option<PathBuf>,
) -> Result {
    std::env::set_current_dir(&*TEMP_DIR)?;

    let data_fetched = if let Some(file) = maybe_file {
        fetch_from_local_file(base_url, &file, &trunk_project_name).await?
    } else {
        fetch_archive_from_registry(base_url, &trunk_project_name).await?
    };

    let archive_written = DebPackager::build_deb(data_fetched, &export_dir).await?;
    println!("Wrote archive at {}", archive_written.display());

    Ok(())
}

async fn fetch_from_local_file(
    base_url: String,
    archive_path: &Path,
    trunk_project_name: &str,
) -> Result<FetchData> {
    let (_client, extension) = fetch_extension(base_url, trunk_project_name).await?;

    let archive = std::fs::read(archive_path).with_context(|| "Failed to read supplied archive")?;

    Dependencies::decompress_archive(extension, &archive)
}

async fn fetch_extension(
    base_url: String,
    trunk_project_name: &str,
) -> Result<(Client, Extension)> {
    let client = Client::new(base_url);

    let extensions = client
        .fetch_extensions()
        .await
        .with_context(|| "Failed to fetch extensions")?;

    let extension = extensions
        .into_iter()
        .find(|ext| ext.name == trunk_project_name)
        .with_context(|| {
            format!("Failed to find a Trunk project with name {trunk_project_name}")
        })?;

    Ok((client, extension))
}

async fn fetch_archive_from_registry(
    base_url: String,
    trunk_project_name: &str,
) -> Result<FetchData> {
    let (client, extension) = fetch_extension(base_url, trunk_project_name).await?;

    Dependencies::fetch_from_archive(extension, client)
        .await
        .with_context(|| "Failed to fetch archive")
}

async fn package_all_extensions(base_url: String, export_dir: PathBuf) -> Result {
    let export_dir: Arc<Path> = Arc::from(export_dir);
    let client = Client::new(base_url);
    std::env::set_current_dir(&*TEMP_DIR)?;

    let extensions = client.fetch_extensions().await?;
    let mut handles = Vec::with_capacity(extensions.len());

    println!(
        "[{}] Loaded {} extensions.",
        "yay!".green(),
        extensions.len().blue()
    );

    for extension in extensions {
        // Copies for the Tokio Task
        let my_client = client.clone();
        let my_export_dir = export_dir.clone();

        let work = async move {
            let data_fetched = Dependencies::fetch_from_archive(extension, my_client).await?;

            let archive_written = DebPackager::build_deb(data_fetched, my_export_dir).await?;
            println!("Wrote archive at {}", archive_written.display());

            Ok(())
        };

        handles.push(tokio::spawn(work));
    }

    let mut failing_extensions = Vec::with_capacity(24);

    for handle in handles {
        if let Err(failing_extension) = handle.await? {
            failing_extensions.push(failing_extension);
        }
    }

    for failing_extension in failing_extensions {
        eprintln!("Err: {failing_extension}");
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result {
    match cli::parse_args() {
        Subcommands::ShowSharedObjects(_) => Ok(()),
        Subcommands::PackageAll(PackageAll {
            base_url,
            export_dir,
        }) => package_all_extensions(base_url, export_dir).await,
        Subcommands::PackageOne(PackageOne {
            base_url,
            trunk_project_name,
            export_dir,
            file,
        }) => {
            let export_dir = std::fs::canonicalize(export_dir)?;
            package_extension(base_url, trunk_project_name, export_dir, file).await
        }
    }
}
