use std::{io::Read, path::Path};

use crate::Result;

use anyhow::Context;
use fs::File;
use fs_err as fs;

pub fn read_to_vec(path: &Path) -> Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();

    let mut buf = Vec::with_capacity(length as usize);

    file.read_to_end(&mut buf)?;

    Ok(buf)
}

pub fn path_to_str(path: &Path) -> Result<&str> {
    path.to_str()
        .with_context(|| "Expected path to be valid UTF-8")
}
