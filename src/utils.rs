use std::{io::Read, path::Path};

use crate::Result;

use fs::File;
use fs_err as fs;

pub fn read_to_vec(path: &Path) -> Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();

    let mut buf = Vec::with_capacity(length as usize);

    file.read_to_end(&mut buf)?;

    Ok(buf)
}