use std::ffi::OsStr;
use std::io::Read;
use std::os::unix::prelude::OsStrExt;
use std::path::Path;
use std::{io::Write, path::PathBuf};

use flate2::Compression;
use fs_err::File;

use crate::dependencies::{DependencySupplier, FetchData};
use crate::unarchiver::Unarchiver;
use crate::{client::Extension, dependencies::Dependencies, EXPORT_DIR};
use crate::{split_newlines, Result, TEMP_DIR};

pub struct DebPackage {
    builder: ar::Builder<File>,
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
    fn gzip(path: &Path) -> Result<Vec<u8>> {
        let uncompressed_bytes = {
            let mut buf = Vec::with_capacity(2048);
            File::open(path)?.read_to_end(&mut buf)?;

            buf
        };

        let compressed_bytes = {
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::with_capacity(2048), Compression::default());

            encoder.write_all(&uncompressed_bytes)?;
            encoder.finish()?
        };

        Ok(compressed_bytes)
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

        Self::gzip(tar_file.path())
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
        let mut file = File::create(&control_path)?;

        // TODO: save as something else? perhaps "postgres15-{extension-name}-trunk"
        writeln!(file, "Package: {}", extension.name)?;
        writeln!(file, "Section: database")?;
        writeln!(file, "Architecture: amd64")?;
        writeln!(file, "Version: {}", extension.latest_version)?;
        writeln!(file, "Description: {}", extension.description)?;
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

    pub async fn build_deb(
        FetchData {
            extension,
            dependencies,
            archive_file,
        }: FetchData,
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

        let archive_path = EXPORT_DIR.join(format!("{}.deb", extension.name));
        let mut deb_archive = DebPackage::new(&archive_path)?;
        deb_archive.add_file("debian-binary", b"2.0\n")?;

        // Save the `control` file to our temp directory
        let tar_gzipped = DebPackager::write_control_file(&extension, &dependencies)?;
        deb_archive.add_file("control.tar.gz", &tar_gzipped)?;

        // Go through each file in the archive and save it to the `deb` folder
        let tar_gzipped = DebPackager::write_packaged_files(&archive_file).await?;
        deb_archive.add_file("data.tar.gz", &tar_gzipped)?;

        Ok(archive_path)
    }

    async fn write_packaged_files(archive_file: &Path) -> Result<Vec<u8>> {
        // Will be `data.tar.gz`
        let tar_file = tempfile::NamedTempFile::new()?;
        let mut builder = tar::Builder::new(tar_file.as_file());

        let save_to = TEMP_DIR.path();
        let stdout = Unarchiver::extract_all(archive_file, save_to).await?;

        let files_extracted = split_newlines(&stdout);

        for path in files_extracted {
            let maybe_extension = path.extension().map(OsStr::as_bytes);
            let mut file = File::open(path)?;

            match maybe_extension {
                Some(b"control") | Some(b"sql") => {
                    let target = format!("usr/share/postgresql/15/{}", path.display());

                    builder.append_file(target, file.file_mut())?;
                }
                Some(b"json") => {
                    // TODO: I don't know if these should go somewhere
                }
                Some(b"so") => {
                    let target = format!("usr/share/postgresql/15/lib/{}", path.display());

                    builder.append_file(target, file.file_mut())?;
                }
                Some(b"bc") => {
                    let target = format!("usr/lib/postgresql/15/lib/{}", path.display());

                    builder.append_file(target, file.file_mut())?;
                }
                Some(_) | None => {
                    // If the file had no extension, or another one, then it's likely a license file
                }
            }
        }

        Self::gzip(tar_file.path())
    }
}
