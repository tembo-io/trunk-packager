mod client;
mod dependencies;
mod shared_lib_registry;
mod unarchiver;

use std::io::Write;
use std::ops::Not;
use std::{
    fs::File,
    io::{self},
    sync::Arc,
};

pub use shared_lib_registry::SharedLibraryRegistry;
use anyhow::{Context, Ok};
use memmap::MmapOptions;
use owo_colors::OwoColorize;

use crate::dependencies::Dependencies;
use crate::{client::Client, unarchiver::Unarchiver};

pub type Result<T = ()> = anyhow::Result<T>;

async fn fetch_and_print_dependencies(client: Client, extension_name: Arc<str>) -> Result {
    fn print_to_stdout(
        extension_name: &str,
        dependencies: &Dependencies
    ) -> Result {
        let mut stdout = io::stdout().lock();

        writeln!(
            stdout,
            "- Libraries for {extension_name}:\n{dependencies}",
        )?;
        stdout.flush()?;

        Ok(())
    }

    // Get the archive for this extension
    let archive_file = client.fetch_extension_archive(&extension_name).await?;
    let mut dependencies = Dependencies::new();

    // The output from the `tar` binary after it decompressed `.so` files from the archive
    let decompression_stdout = Unarchiver::extract_shared_objs(&archive_file).await?;
    let shared_objects = decompression_stdout
        .split('\n')
        .filter(|file| file.is_empty().not());

    for object in shared_objects {
        let file = File::open(&object)?;
        let map = unsafe { MmapOptions::new().map(&file) }?;

        let obj = goblin::Object::parse(&map)?;
        let shared_libraries = match obj {
            goblin::Object::Elf(elf) => elf.libraries,
            other => {
                eprintln!(
                    "{} has an unsupported object format: {:?}",
                    extension_name, other
                );
                continue;
            }
        };

        for library in &shared_libraries {
            dependencies.add(library);
        }
    }

    print_to_stdout(&*extension_name, &dependencies)?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result {
    let client = Client::new();

    let previous_dir = std::env::current_dir()?;
    std::env::set_current_dir(client.temp_dir())?;

    let extensions = client.fetch_extensions().await?;
    let mut handles = Vec::with_capacity(extensions.len());

    println!(
        "[{}] Loaded {} extensions.",
        "yay!".green(),
        extensions.len().blue()
    );

    for extension in extensions {
        let extension: Arc<str> = Arc::from(extension.name);

        // Copies for the Tokio Task
        let my_client = client.clone();
        let my_extension = extension.clone();

        let work = async move {
            fetch_and_print_dependencies(my_client, my_extension.clone())
                .await
                .with_context(|| my_extension)
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
        println!("Failed on {failing_extension}");
    }

    std::env::set_current_dir(previous_dir)?;

    Ok(())
}
