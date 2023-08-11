use std::{fs::File, path::Path, sync::Arc};

use crate::Result;

use dashmap::DashSet;
use phf::{phf_map, Map};
use std::io::Write;

static DEPENDENCY_SUPPLIERS: Map<&'static str, &'static str> = phf_map! {
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

#[derive(Clone)]
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
