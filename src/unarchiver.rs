use std::{
    ffi::OsStr,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    process::Stdio,
    time::{SystemTime, UNIX_EPOCH},
};

use bytes::Bytes;
use flate2::read::GzDecoder;
use tar::EntryType;
use tokio::process::Command;

use crate::{utils::path_to_str, Result};

pub struct Unarchiver;

pub struct Archive {
    entries: Vec<Entry>,
}

impl Archive {
    pub fn shared_objects(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(|entry| entry.is_shared_object())
    }

    pub fn all_entries(&self) -> &[Entry] {
        &self.entries
    }
}

pub struct Entry {
    pub path: PathBuf,
    pub contents: Vec<u8>,
}

impl Entry {
    pub fn is_shared_object(&self) -> bool {
        matches!(self.extension(), Some(b"so"))
    }

    pub fn extension(&self) -> Option<&[u8]> {
        use std::os::unix::ffi::OsStrExt;

        self.path.extension().map(OsStr::as_bytes)
    }

    pub fn tar_header(&self) -> tar::Header {
        let mut header = tar::Header::new_gnu();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

        header.set_mode(0o644);
        header.set_mtime(now.as_secs());
        header.set_uid(0);
        header.set_gid(0);
        header.set_size(self.contents.len() as u64);
        header.set_entry_type(EntryType::Regular);
        
        header.set_cksum();
        header
    }
}

impl Unarchiver {
    pub async fn decompress_in_memory(tar_gz: Bytes) -> Result<Archive> {
        let mut buf = Vec::with_capacity(tar_gz.len() * 8);
        GzDecoder::new(tar_gz.as_ref()).read_to_end(&mut buf)?;

        let mut archive = tar::Archive::new(Cursor::new(buf));

        let mut entries = Vec::new();

        for maybe_entry in archive.entries()? {
            let mut entry = maybe_entry?;

            let path = entry.path()?.to_path_buf();

            let contents = {
                let entry_size = entry.header().entry_size().unwrap_or(12500);
                let mut buf = Vec::with_capacity(entry_size as usize);

                entry.read_to_end(&mut buf)?;
                buf
            };

            entries.push(Entry { path, contents });
        }

        Ok(Archive { entries })
    }

    #[allow(unused)]
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

    #[allow(unused)]
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

    #[allow(unused)]
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
