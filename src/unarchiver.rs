use std::{
    ffi::OsStr,
    io::{Cursor, Read},
    path::PathBuf,
};

use bytes::Bytes;
use flate2::read::GzDecoder;
use tar::EntryType;

use crate::Result;

pub struct Unarchiver;

pub struct Archive {
    entries: Vec<Entry>,
}

impl Archive {
    pub fn shared_objects(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(|entry| entry.is_shared_object())
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

}

impl Unarchiver {
    pub async fn decompress_in_memory(tar_gz: Bytes) -> Result<Archive> {
        let mut buf = Vec::with_capacity(tar_gz.len() * 8);
        GzDecoder::new(tar_gz.as_ref()).read_to_end(&mut buf)?;

        let mut archive = tar::Archive::new(Cursor::new(buf));

        let mut entries = Vec::new();

        for maybe_entry in archive.entries()? {
            let mut entry = maybe_entry?;
            let header = entry.header();
            let entry_size = header.entry_size().unwrap_or(12500);

            match header.entry_type() {
                EntryType::Regular => {}
                other => {
                    eprintln!(
                        "decompressing: Found a {:?} file, expected Regular. Ignoring",
                        other
                    );
                    continue;
                }
            }

            let path = entry.path()?.into();

            let contents = {
                let mut buf = Vec::with_capacity(entry_size as usize);

                entry.read_to_end(&mut buf)?;
                buf
            };

            entries.push(Entry { path, contents });
        }

        Ok(Archive { entries })
    }
}
