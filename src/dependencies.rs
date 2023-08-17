use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    sync::Arc,
};

use owo_colors::OwoColorize;
use phf::{phf_map, phf_set, Map};

use crate::{
    client::{Client, Extension},
    unarchiver::Archive,
};
use crate::{unarchiver::Unarchiver, Result};

/// Shared libraries supplied by libc
static BASIC_SHARED_LIBS: phf::Set<&'static str> = phf_set! {
    "libm.so.6",
    "ld-linux.so.2",
    "ld-linux-x86-64.so.2"
};

static DEPENDENCY_SUPPLIERS: Map<&'static str, &'static str> = phf_map! {
    "libc.so.6" => "libc6",
    "libstdc++.so.6" => "libstdc++6",
    "libR.so" => "r-base-core",
    "libcrypto.so.3" => "openssl",
    "liblz4.so.1" => "liblz4-1",
    "libgeos_c.so.1" => "libgeos-c1v5",
    "libtcl8.6.so" => "libtcl8.6.so",
    "libpcre2-8.so.0" => "libpcre2-8-0",
    "libhiredis.so.0.14" => "libhiredis0.14",
    "libuuid.so.1" => "libuuid1",
    "libgroonga.so.0" => "libgroonga0",
    "libopenblas.so.0" => "libopenblas0-pthread",
    "libcurl.so.4" => "libcurl4",
    "libpython3.10.so.1." => "libpython3.10",
    "libjson-c.so.5" => "libjson-c5",
    "libsybdb.so.5" => "libsybdb5",
    "libsodium.so.23" => "libsodium23",
    "libboost_serialization.so.1.74.0" => "libboost-serialization1.74.0",
    "libgcc_s.so.1" => "libgcc-s1",
    "libxml2.so.2" => "libxml2",
    "libselinux.so.1" => "libselinux1",
    "libprotobuf-c.so.1" => "libprotobuf-c1",
    "librdkafka.so.1" => "librdkafka1",
    "libgdal.so.30" => "libgdal30",
    "libcrypt.so.1" => "libcrypt1",
    "libpq.so.5" => "libpq5",
    "liburiparser.so.1" => "liburiparser1",
    "libfreetype.so.6" => "libfreetype6",
    "libzstd.so.1" => "libzstd1",
    "libz.so.1" => "zlib1g",
    "libperl.so.5.34" => "libperl5.34",
    "libgomp.so.1" => "libgomp1",
    "libssl.so.3" => "libssl3",
    "libproj.so.22" => "libproj22",
    "libSFCGAL.so.1" => "libsfcgal1",
};

#[derive(Hash, Clone, Copy)]
pub enum DependencySupplier {
    MetBy { package: &'static str },
    Unknown,
}

impl DependencySupplier {
    pub fn is_met(&self) -> bool {
        matches!(self, Self::MetBy { package: _ })
    }

    pub fn name(&self) -> &str {
        match self {
            DependencySupplier::MetBy { package } => package,
            DependencySupplier::Unknown => "<unknown>",
        }
    }
}

impl Display for DependencySupplier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DependencySupplier::MetBy { package } => write!(f, "{}", package.green()),
            DependencySupplier::Unknown => write!(f, "{}", "(unknown)".red()),
        }
    }
}

pub struct Dependencies {
    pub shared_libraries: HashSet<Arc<str>>,
    pub suppliers: HashMap<Arc<str>, DependencySupplier>,
}

impl Display for Dependencies {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for library in &self.shared_libraries {
            let supplier = &self.suppliers[library];
            writeln!(f, "\t{library} met by {supplier}")?
        }

        Ok(())
    }
}

pub struct FetchData {
    /// Actual extension data
    pub extension: Extension,
    /// The system dependencies of this file
    pub dependencies: Dependencies,
    /// The decompressed contents of the .tar.gz archive downloaded from Trunk
    pub archive: Archive,
}

impl Dependencies {
    /// Fetch an extension's dependencies by analyzing its compiled archive
    pub async fn fetch_from_archive(extension: Extension, client: Client) -> Result<FetchData> {
        let mut dependencies = Self::new();

        // Get the archive for this extension
        let tar_gz = client.fetch_extension_archive(&extension.name).await?;
        let archive = Unarchiver::decompress_in_memory(tar_gz).await?;

        for entry in archive.shared_objects() {
            let obj = goblin::Object::parse(&entry.contents)?;
            let shared_libraries = match obj {
                goblin::Object::Elf(elf) => elf.libraries,
                other => {
                    eprintln!(
                        "{} has an unsupported object format: {:?}",
                        extension.name, other
                    );
                    continue;
                }
            };

            for library in &shared_libraries {
                dependencies.add(library);
            }
        }

        Ok(FetchData {
            extension,
            dependencies,
            archive,
        })
    }

    pub fn new() -> Self {
        Self {
            shared_libraries: HashSet::with_capacity(8),
            suppliers: HashMap::with_capacity(8),
        }
    }

    pub fn add(&mut self, mut shared_object: &str) {
        if self.shared_libraries.contains(shared_object) {
            // Dependency was already inserted, no more work to do
            return;
        }
        if BASIC_SHARED_LIBS.contains(shared_object) {
            shared_object = "libc.so.6";
        }

        let supplier = DEPENDENCY_SUPPLIERS
            .get(shared_object)
            .map(|package| DependencySupplier::MetBy { package })
            .unwrap_or(DependencySupplier::Unknown);

        let owned: Arc<str> = Arc::from(shared_object);

        self.shared_libraries.insert(owned.clone());
        self.suppliers.insert(owned, supplier);
    }
}
