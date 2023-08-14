use std::ffi::OsStr;
use std::os::unix::prelude::OsStrExt;
use std::path::Path;
use std::{io::Write, path::PathBuf};

use fs_err::{self as fs, File};

use crate::dependencies::{DependencySupplier, FetchData};
use crate::unarchiver::Unarchiver;
use crate::{client::Extension, dependencies::Dependencies, EXPORT_DIR};
use crate::{split_newlines, Result, TEMP_DIR};

pub enum DebPackager {}

impl DebPackager {
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
    fn write_control_file<W: Write>(
        extension: &Extension,
        dependencies: &Dependencies,
        builder: &mut tar::Builder<W>,
    ) -> Result<()> {
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

        dbg!(Path::new(&file_name).exists());
        builder.append_path(&file_name)?;

        Ok(())
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
        let output_archive = { std::fs::File::create(&archive_path)? };

        let mut builder = tar::Builder::new(output_archive);

        // Save the `control` file to our temp directory
        DebPackager::write_control_file(&extension, &dependencies, &mut builder)?;
        // Go through each file in the archive and save it to the `deb` folder
        DebPackager::write_packaged_files(&archive_file, &mut builder).await?;

        builder.finish()?;
        Ok(archive_path)
    }

    async fn write_packaged_files<W: Write>(
        archive_file: &Path,
        builder: &mut tar::Builder<W>,
    ) -> Result {
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

        Ok(())
    }
}
