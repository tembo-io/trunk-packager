use std::{ops::Not, path::Path, process::Stdio};

use anyhow::Context;
use tokio::process::Command;

use crate::Result;

pub struct Unarchiver;

impl Unarchiver {
    pub async fn extract_shared_objs(path: &Path) -> Result<Vec<String>> {
        let file_name = path
            .to_str()
            .with_context(|| "Expected path to contain a file name")?;

        let arguments = ["-xzvf", file_name, "--wildcards", "*.so"];

        // Take from the archive only .so files
        let output = Command::new("tar")
            .args(arguments)
            .stdout(Stdio::piped())
            .spawn()?
            .wait_with_output()
            .await?;

        if output.status.success() {
            // In stdout we'll find the files that were decompressed
            let utf8_output = String::from_utf8(output.stdout)?;

            Ok(utf8_output
                .split('\n')
                .filter(|file| file.is_empty().not())
                .map(ToOwned::to_owned)
                .collect())
        } else {
            anyhow::bail!("Failed to decompress archive");
        }
    }
}