use std::{fs::File, path::Path, sync::Arc};

use crate::Result;

use dashmap::DashSet;
use std::io::Write;

pub struct SharedLibraryRegistry {
    set: Arc<DashSet<Box<str>>>,
}

impl SharedLibraryRegistry {
    pub fn new() -> Self {
        Self {
            set: Arc::new(DashSet::with_capacity(64)),
        }
    }

    pub fn extend(&self, libraries: &[&str]) {
        for library in libraries {
            self.set.insert(Box::from(*library));
        }
    }

    pub fn export(&self, path: impl AsRef<Path>) -> Result {
        let mut file = File::create(path)?;

        for library in self.set.iter() {
            writeln!(file, "{}", *library)?;
        }

        Ok(())
    }
}
