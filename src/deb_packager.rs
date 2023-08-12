use std::ops::Not;
use std::path::Path;
use std::{fs::File, io::Write, path::PathBuf};

use tempfile::tempdir;

use crate::dependencies::DependencySupplier;
use crate::Result;
use crate::{client::Extension, dependencies::Dependencies, EXPORT_DIR};

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
    fn write_control_file(
        extension: &Extension,
        dependencies: &Dependencies,
        dir: &Path,
    ) -> Result<()> {
        let mut file = {
            let path = dir.join(format!("{}-{}.control", extension.name, extension.latest_version));

            File::create(path)?
        };

        // TODO: save as something else? perhaps "postgres15-{extension-name}-trunk"
        writeln!(file, "Package: {}", extension.name)?;
        writeln!(file, "Section: database")?;
        writeln!(file, "Architecture: amd64")?;
        writeln!(file, "Version: {}", extension.latest_version)?;
        writeln!(file, "Description: {}", extension.description)?;

        // Write down the dependencies
        Self::write_dependencies(&mut file, dependencies)?;

        Ok(())
    }

    pub fn build_deb(extension: Extension, dependencies: Dependencies) -> Result<PathBuf> {
        // Check if this .deb is actually writable (e.g. if we know all dependencies it requires)
        let all_dependencies_are_known = dependencies
            .suppliers
            .values()
            .all(DependencySupplier::is_met);
        anyhow::ensure!(
            all_dependencies_are_known,
            "The supplied for some/all dependencies of {} are unknown",
            extension.name
        );

        let output_file = {
            let file_name = format!(
                "postgres15-{}-{}.deb",
                extension.name, extension.latest_version
            );

            EXPORT_DIR.join(file_name)
        };

        // Will contain the necessary .deb files
        // let temp_dir = tempdir()?;

        // Save the `control` file to our temp directory
        DebPackager::write_control_file(&extension, &dependencies, &*EXPORT_DIR)?;

        Ok(output_file)
    }
}
