use anyhow::{anyhow, Result};
use clap::ArgMatches;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub mode: Mode,
    pub fstype: FilesystemType,
    pub extracted_dir: PathBuf,
    pub file_contexts: PathBuf,
    pub cores: usize,
    pub silent: bool,
    pub vendor_bin_context: String,
    pub system_bin_context: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    All,
    Bin,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FilesystemType {
    Ext4,
    Erofs,
    F2fs,
}

impl FilesystemType {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "ext4" => Ok(Self::Ext4),
            "erofs" => Ok(Self::Erofs),
            "f2fs" => Ok(Self::F2fs),
            _ => Err(anyhow!("Unsupported filesystem type: {}", s)),
        }
    }

    pub fn folder_pattern(&self) -> &'static str {
        match self {
            Self::Ext4 => "(/.*)?",
            Self::Erofs | Self::F2fs => "",
        }
    }
}

impl Config {
    pub fn from_matches(matches: &ArgMatches) -> Result<Self> {
        let mode = if matches.get_flag("bin") {
            Mode::Bin
        } else if matches.get_flag("all") {
            Mode::All
        } else {
            return Err(anyhow!("Must specify either -a or -b mode"));
        };

        let fstype = FilesystemType::from_str(
            matches.get_one::<String>("fstype").unwrap()
        )?;

        let extracted_dir = PathBuf::from(matches.get_one::<String>("partition").unwrap());
        let file_contexts = PathBuf::from(matches.get_one::<String>("contexts").unwrap());

        let cores = matches.get_one::<String>("threads")
            .unwrap()
            .parse::<usize>()
            .map_err(|_| anyhow!("Invalid thread count"))?;

        let silent = matches.get_flag("quiet");

        let format_context = |s: &str| -> String {
            if s.contains(':') {
                s.to_string()
            } else {
                format!("u:object_r:{}:s0", s)
            }
        };

        let vendor_bin_context = matches.get_one::<String>("vbin")
            .map(|s| format_context(s))
            .unwrap_or_else(|| "u:object_r:vendor_qti_init_shell_exec:s0".to_string());

        let system_bin_context = matches.get_one::<String>("sbin")
            .map(|s| format_context(s))
            .unwrap_or_else(|| "u:object_r:system_file:s0".to_string());

        if !extracted_dir.exists() {
            return Err(anyhow!("Partition directory does not exist: {:?}", extracted_dir));
        }
        Ok(Self {
            mode,
            fstype,
            extracted_dir,
            file_contexts,
            cores,
            silent,
            vendor_bin_context,
            system_bin_context,
        })
    }
}