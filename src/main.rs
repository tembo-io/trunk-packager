mod client;
mod unarchiver;

use std::{fs::File, sync::Arc, io::{ self}};
use std::io::Write;

use anyhow::Context;
use memmap::MmapOptions;
use owo_colors::OwoColorize;

use crate::{client::Client, unarchiver::Unarchiver};

pub type Result<T = ()> = anyhow::Result<T>;

async fn print_shared_libraries(client: Client, extension_name: Arc<str>) -> Result {
    // Get the archive for this extension
    let archive_file = client.fetch_extension_archive(&extension_name).await?;
    
    let shared_objects = Unarchiver::extract_shared_objs(&archive_file).await?;

    for object in shared_objects {
        let file = File::open(&object)?;
        let map = unsafe { MmapOptions::new().map(&file) }?;

        let obj = goblin::Object::parse(&map)?;
        let shared_libraries = match obj {
            goblin::Object::Elf(elf) => elf.libraries,
            other => {
                eprintln!("{} has an unsupported object format: {:?}", extension_name, other);
                continue;
            }
        };

        let mut stdout = io::stdout().lock();

        writeln!(stdout, "- Libraries for {} ({})", extension_name.green(), object)?;
        for library in shared_libraries {
            writeln!(stdout, "\t* - {library}")?;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result {
    let client = Client::new();
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

        let my_client = client.clone();
        let my_extension = extension.clone();

        let work = async move {
            print_shared_libraries(my_client, my_extension.clone()).await.with_context(|| my_extension)
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

    Ok(())
}
