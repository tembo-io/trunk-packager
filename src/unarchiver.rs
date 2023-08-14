use std::{path::Path, process::Stdio};

use tokio::process::Command;

use crate::{utils::path_to_str, Result};

pub struct Unarchiver;

impl Unarchiver {
    async fn ensure_has_shared_objects(file_name: &str) -> Result<()> {
        let arguments = ["-tf", file_name, "--wildcards", "*.so"];

        let output = Command::new("tar")
            .args(arguments)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?
            .wait()
            .await?;

        anyhow::ensure!(
            output.success(),
            "{file_name} did not contain a shared object"
        );
        Ok(())
    }

    pub async fn extract_shared_objs(path: &Path, save_to: &Path) -> Result<String> {
        let file_name = path_to_str(path)?;
        let export_to = path_to_str(save_to)?;

        Self::ensure_has_shared_objects(file_name).await?;

        let arguments = ["-xzvf", file_name, "-C", export_to, "--wildcards", "*.so"];

        // Take from the archive only .so files
        let output = Command::new("tar")
            .args(arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?
            .wait_with_output()
            .await?;

        if output.status.success() {
            // In stdout we'll find the files that were decompressed
            let utf8_output = String::from_utf8(output.stdout)?;

            Ok(utf8_output)
        } else {
            let utf8_output = String::from_utf8(output.stderr)?;

            anyhow::bail!("Failed to decompress archive: {utf8_output}");
        }
    }

    pub async fn extract_all(path: &Path, save_to: &Path) -> Result<String> {
        let file_name = path_to_str(path)?;
        let export_to = path_to_str(save_to)?;

        let arguments = ["-xzvf", file_name, "-C", export_to];

        // Take from the archive only .so files
        let output = Command::new("tar")
            .args(arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?
            .wait_with_output()
            .await?;

        if output.status.success() {
            // In stdout we'll find the files that were decompressed
            let utf8_output = String::from_utf8(output.stdout)?;

            Ok(utf8_output)
        } else {
            let utf8_output = String::from_utf8(output.stderr)?;

            anyhow::bail!("Failed to decompress archive: {utf8_output}");
        }
    }
}
