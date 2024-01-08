use std::collections::HashSet;
use std::io::Cursor;
use std::ops::Not;
use std::path::{Component, Path};
use std::{io::Write, path::PathBuf};

use anyhow::Ok;
use flate2::Compression;
use fs_err::File;

use crate::dependencies::{DependencySupplier, FetchData};
use crate::unarchiver::{Archive, Entry};
use crate::{client::Extension, dependencies::Dependencies};
use crate::{utils, Result, TEMP_DIR};

pub struct DebPackage {
    builder: ar::Builder<File>,
}

pub struct TarArchive {
    directories_created: HashSet<PathBuf>,
    builder: tar::Builder<Vec<u8>>,
}

impl TarArchive {
    pub fn new() -> Self {
        let buf = Vec::new();
        let builder = tar::Builder::new(buf);

        Self {
            builder,
            directories_created: HashSet::new(),
        }
    }

    fn add_directory(&mut self, path: &Path) -> Result<()> {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Directory);
        header.set_size(0);
        header.set_mode(0o755);
        header.set_gid(0);
        header.set_uid(0);
        header.set_cksum();

        self.builder
            .append_data(&mut header, path, &mut std::io::empty())?;

        Ok(())
    }

    fn populate_ancestor_paths(&mut self, path: &Path) -> Result<()> {
        let mut directory = PathBuf::with_capacity(20);
        directory.push("./");

        for comp in path.components() {
            match comp {
                Component::CurDir => {}
                Component::Normal(c) => directory.push(c),
                _ => continue,
            }

            if self.directories_created.contains(&directory).not() {
                self.add_directory(&directory)?;

                self.directories_created.insert(directory.clone());
            }
        }

        Ok(())
    }

    pub fn add_entry<P: AsRef<Path>>(&mut self, entry: &Entry, path: &P) -> Result<()> {
        let path = path.as_ref();
        let parent = path.parent().unwrap();
        self.populate_ancestor_paths(parent)?;

        let mut header = entry.tar_header();
        let contents = Cursor::new(&entry.contents);

        self.builder.append_data(&mut header, path, contents)?;

        Ok(())
    }

    pub fn into_bytes(mut self) -> Result<Vec<u8>> {
        self.builder.finish()?;

        Ok(self.builder.into_inner()?)
    }
}

impl DebPackage {
    pub fn new(path: &Path) -> Result<Self> {
        let file = File::create(path)?;
        let builder = ar::Builder::new(file);

        Ok(Self { builder })
    }

    pub fn add_file(&mut self, path: impl AsRef<[u8]>, data: &[u8]) -> Result {
        let identifier_bytes = path.as_ref().into();
        let mut header = ar::Header::new(identifier_bytes, data.len() as u64);
        header.set_mode(0o644);
        // TODO: set modification time
        header.set_mtime(0);
        header.set_uid(0);
        header.set_gid(0);

        self.builder.append(&header, data)?;

        Ok(())
    }
}

pub enum DebPackager {}

impl DebPackager {
    fn gzip_bytes(bytes: &[u8]) -> Result<Vec<u8>> {
        let compressed_bytes = {
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::with_capacity(2048), Compression::default());

            encoder.write_all(bytes)?;
            encoder.finish()?
        };

        Ok(compressed_bytes)
    }

    fn gzip_path(path: &Path) -> Result<Vec<u8>> {
        let uncompressed_bytes = utils::read_to_vec(path)?;

        Self::gzip_bytes(&uncompressed_bytes)
    }

    /// Return the .tar.gz  bytes of the file on the given path
    fn tar_gzip(path: &Path) -> Result<Vec<u8>> {
        let tar_file = tempfile::NamedTempFile::new()?;

        let mut builder = tar::Builder::new(tar_file.as_file());
        {
            let mut control_file = File::open(path)?;
            builder.append_file("control", control_file.file_mut())?;
            builder.finish()?;
        }

        Self::gzip_path(tar_file.path())
    }

    fn write_dependencies(file: &mut File, dependencies: &Dependencies) -> Result {
        let suppliers = dependencies.suppliers.values();
        if suppliers.len() == 0 {
            return Ok(());
        }
        let last_idx = suppliers.len();

        write!(file, "Depends: ")?;

        // TODO: show dependency versions
        for (idx, supplier) in suppliers.enumerate() {
            write!(file, "{}", supplier.name())?;
            if idx + 1 != last_idx {
                write!(file, ", ")?;
            }
        }

        writeln!(file)?;

        Ok(())
    }

    /// Writes the .deb control file
    ///
    /// Docs.:
    fn write_control_file(extension: &Extension, dependencies: &Dependencies) -> Result<Vec<u8>> {
        let file_name = format!("{}-{}.control", extension.name, extension.latest_version);
        let control_path = TEMP_DIR.path().join(&file_name);
        let mut file = File::create(control_path)?;

        // TODO: save as something else? perhaps "postgres15-{extension-name}-trunk"
        writeln!(file, "Package: {}", extension.name)?;
        writeln!(file, "Section: database")?;
        writeln!(file, "Architecture: amd64")?;
        writeln!(file, "Version: {}", extension.latest_version)?;
        writeln!(
            file,
            "Description: {}",
            extension.description.as_deref().unwrap_or("")
        )?;
        writeln!(
            file,
            "Homepage: https://pgt.dev/extensions/{}",
            extension.name
        )?;

        // Write down the dependencies
        Self::write_dependencies(&mut file, dependencies)?;
        file.flush()?;

        Self::tar_gzip(Path::new(&file_name))
    }

    pub async fn build_deb<P: AsRef<Path>>(
        FetchData {
            extension,
            dependencies,
            archive,
        }: FetchData,
        export_dir: P,
    ) -> Result<PathBuf> {
        // Check if this .deb is actually writable (e.g. if we know all dependencies it requires)
        let all_dependencies_are_known = dependencies
            .suppliers
            .values()
            .all(DependencySupplier::is_met);
        anyhow::ensure!(
            all_dependencies_are_known,
            "The packages that supply the dependencies of {} are unknown",
            extension.name
        );

        let archive_path = export_dir.as_ref().join(format!("{}.deb", extension.name));
        let mut deb_archive = DebPackage::new(&archive_path)?;
        deb_archive.add_file("debian-binary", b"2.0\n")?;

        // Save the `control` file to our temp directory
        let tar_gzipped = DebPackager::write_control_file(&extension, &dependencies)?;
        deb_archive.add_file("control.tar.gz", &tar_gzipped)?;

        // Go through each file in the archive and save it to the `deb` folder
        let tar_gzipped = DebPackager::write_packaged_files(&archive).await?;
        deb_archive.add_file("data.tar.gz", &tar_gzipped)?;

        Ok(archive_path)
    }

    async fn write_packaged_files(archive: &Archive) -> Result<Vec<u8>> {
        let mut data_tar = TarArchive::new();

        for entry in archive.all_entries() {
            let maybe_extension = entry.extension();

            match maybe_extension {
                Some(b"control") | Some(b"sql") => {
                    let target = format!(".//usr/share/postgresql/15/{}", entry.path.display());

                    data_tar.add_entry(entry, &target)?;
                }
                Some(b"json") => {
                    // TODO: I don't know if these should go somewhere
                }
                Some(b"so") => {
                    let target = format!(".//usr/share/postgresql/15/lib/{}", entry.path.display());

                    data_tar.add_entry(entry, &target)?;
                }
                Some(b"bc") => {
                    let target = format!(".//usr/lib/postgresql/15/lib/{}", entry.path.display());

                    data_tar.add_entry(entry, &target)?;
                }
                Some(_) | None => {
                    // If the file had no extension, or another one, then it's likely a license file
                }
            }
        }

        let bytes = data_tar.into_bytes()?;
        Self::gzip_bytes(&bytes)
    }
}
