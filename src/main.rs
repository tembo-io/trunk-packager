mod cli;
mod client;
mod dependencies;
mod unarchiver;
mod utils;

use std::result::Result as StdResult;

use std::ops::Not;
use std::path::Path;

use anyhow::Ok;
use once_cell::sync::Lazy;
use owo_colors::OwoColorize;
use slicedisplay::SliceDisplay;
use tempfile::TempDir;

use crate::client::Client;
use crate::dependencies::{Dependencies, DependencySupplier};

pub type Result<T = ()> = anyhow::Result<T>;

pub static TEMP_DIR: Lazy<TempDir> = Lazy::new(|| tempfile::tempdir().unwrap());

pub fn split_newlines(text: &str) -> impl Iterator<Item = &'_ Path> {
    text.split('\n')
        .filter(|line| line.is_empty().not())
        .map(Path::new)
}

fn collect_system_deps(extension_name: &str, dependencies: &Dependencies) -> Vec<&'static str> {
    let mut deps = Vec::new();
    for library in dependencies.shared_libraries.iter() {
        let DependencySupplier::MetBy { package } = dependencies.suppliers[library] else {
            eprintln!("In extension {}, did not find supplier for {}", extension_name, library);
            continue;
        };

        deps.push(package);
    }

    deps
}

#[tokio::main]
async fn main() -> Result {
    println!("Fetching extensions from Trunk...");

    let client = Client::new("https://registry.pgtrunk.io".into());
    
    let extensions = client.fetch_extensions().await?;

    println!(
        "Done! Loaded {} extensions.",
        extensions.len().blue()
    );

    for extension in extensions {
        let StdResult::Ok(data_fetched) = Dependencies::fetch_from_archive(&extension, &client).await else {
            println!("Failed to get information for {}", extension.name);
            continue;
        };
        let dependencies = data_fetched.dependencies;

        let packages = collect_system_deps(&extension.name, &dependencies);
        if packages.is_empty() {
            continue;
        }

        println!("Manifest for {}", extension.name);
        println!("[dependencies]");
        println!("apt = {}", packages.display());
        println!();
    }

    Ok(())
}
