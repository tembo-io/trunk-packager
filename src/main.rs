mod client;
mod deb_packager;
mod dependencies;
mod shared_lib_registry;
mod unarchiver;

use std::io::Write;
use std::io::{self};
use std::path::PathBuf;

use anyhow::Ok;
use once_cell::sync::Lazy;
use owo_colors::OwoColorize;
pub use shared_lib_registry::SharedLibraryRegistry;
use tempfile::{tempdir, TempDir};

use crate::client::Client;
use crate::deb_packager::DebPackager;
use crate::dependencies::Dependencies;

pub type Result<T = ()> = anyhow::Result<T>;

pub static BASE_URL: Lazy<String> = Lazy::new(|| std::env::var("BASE_URL").unwrap());
pub static EXPORT_DIR: Lazy<PathBuf> = Lazy::new(|| std::env::var_os("EXPORT_DIR").unwrap().into());

fn print_to_stdout(extension_name: &str, dependencies: &Dependencies) -> Result {
    let mut stdout = io::stdout().lock();

    writeln!(stdout, "- Libraries for {extension_name}:\n{dependencies}",)?;
    stdout.flush()?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result {
    let client = Client::new();

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

        let work = async move {
            let (extension, dependencies) =
                Dependencies::fetch_from_archive(extension, my_client).await?;

            let archive_written = DebPackager::build_deb(extension, dependencies)?;
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
