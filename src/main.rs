mod client;
mod unarchiver;

use std::fs::File;

use memmap::MmapOptions;
use owo_colors::OwoColorize;

use crate::{client::Client, unarchiver::Unarchiver};

pub type Result<T = ()> = anyhow::Result<T>;

#[tokio::main]
async fn main() -> Result {
    let client = Client::new();
    std::env::set_current_dir(client.temp_dir())?;

    let exts = client.fetch_extensions().await?;
    println!(
        "[{}] Loaded {} extensions.",
        "yay!".green(),
        exts.len().blue()
    );

    for ext in exts {
        // Get the archive for this extension
        let archive_file = client.fetch_extension_archive(&ext).await?;

        let Ok(shared_objects) = Unarchiver::extract_shared_objs(&archive_file).await else {
            continue;
        };

        for object in shared_objects {
            let file = File::open(&object)?;
            let map = unsafe { MmapOptions::new().map(&file) }?;

            let obj = goblin::Object::parse(&map)?;
            let shared_libraries = match obj {
                goblin::Object::Elf(elf) => elf.libraries,
                other => {
                    eprintln!("{} has an unsupported object format: {:?}", ext.name, other);
                    continue;
                }
            };

            println!("- Libraries for {}", ext.name.green());
            for library in shared_libraries {
                println!("\t* - {library}");
            }
        }
    }

    std::mem::forget(client);

    Ok(())
}
