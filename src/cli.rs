use std::path::PathBuf;

use argh::FromArgs;

#[derive(FromArgs, PartialEq, Debug)]
/// Packages Trunk extensions into .deb files
struct Args {
    #[argh(subcommand)]
    nested: Subcommands,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
pub enum Subcommands {
    ShowSharedObjects(ShowSharedObjects),
    PackageAll(PackageAll),
    PackageOne(PackageOne),
}

#[derive(FromArgs, PartialEq, Debug)]
/// Show all shared objects this extension depends on
#[argh(subcommand, name = "show-all")]
pub struct ShowSharedObjects {
    #[argh(option)]
    /// the base URL of the Trunk provider
    base_url: String,
}

#[derive(FromArgs, PartialEq, Debug)]
/// Package all extensions into .deb
#[argh(subcommand, name = "package-all")]
pub struct PackageAll {
    #[argh(option)]
    /// the base URL of the Trunk provider
    pub base_url: String,
    #[argh(option)]
    /// the directory in which to export the generated packages
    pub export_dir: PathBuf,
}

#[derive(FromArgs, PartialEq, Debug)]
/// Package a single extension into a .deb
#[argh(subcommand, name = "package-one")]
pub struct PackageOne {
    #[argh(option)]
    /// the base URL of the Trunk provider
    pub base_url: String,
    #[argh(positional)]
    /// the Trunk project to be packaged
    pub trunk_project_name: String,
    #[argh(option)]
    /// the directory in which to export the generated package
    pub export_dir: PathBuf,
}

pub fn parse_args() -> Subcommands {
    let args: Args = argh::from_env();

    args.nested
}
