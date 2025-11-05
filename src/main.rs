mod config;
mod processor;
mod progress;
mod utils;

use anyhow::Result;
use clap::Command;
use config::Config;

fn main() -> Result<()> {
    let matches = Command::new("file_contexts_generator")
        .about("A tool to automatically generate missing file_contexts based on file/folder location")
        .author("Danda420")
        .arg(clap::arg!(-a --all "Autogenerate all missing contexts").conflicts_with("bin"))
        .arg(clap::arg!(-b --bin "Autogenerate only /bin/ missing contexts").conflicts_with("all"))
        .arg(clap::arg!(-f --fstype <FSTYPE> "Filesystem type: ext4, erofs, f2fs").required(true))
        .arg(clap::arg!(-p --partition <PARTITION> "Path to extracted partition folder").required(true))
        .arg(clap::arg!(-c --contexts <CONTEXTS> "Path to partition_file_contexts file").required(true))
        .arg(clap::arg!(-t --threads <THREADS> "Number of parallel threads to use").default_value("4"))
        .arg(clap::arg!(-q --quiet "Make file_contexts generator quiet"))
        .get_matches();

    let config = Config::from_matches(&matches)?;
    processor::process_file_contexts(&config)
}