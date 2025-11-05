use crate::config::Config;
use crate::progress::bar::ProgressTracker;
use crate::utils::regex_utils::escape_regex;
use anyhow::Result;
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::thread;
use walkdir::WalkDir;

pub fn process_file_contexts(config: &Config) -> Result<()> {
    let partition_name = get_partition_name(&config.extracted_dir);
    let existing_contexts = read_existing_contexts(&config.file_contexts)?;
    let files_to_process = collect_files_to_process(config)?;
    let total_files = files_to_process.len();
    let missing_count = count_missing_entries(&files_to_process, config, &partition_name, &existing_contexts)?;

    if !config.silent {
        let mode_str = match config.mode {
            crate::config::Mode::All => "file_contexts",
            crate::config::Mode::Bin => "/bin/ file_contexts",
        };
        
        if missing_count == 0 {
            println!("No missing entries found in {}.", mode_str);
            println!();
            return Ok(());
        } else {
            println!("{} missing entries detected in {}, autogenerating...", missing_count, mode_str);
        }
    }

    let progress_tracker = ProgressTracker::new(total_files, !config.silent && missing_count > 0);
    
    let chunk_size = (total_files + config.cores - 1) / config.cores;
    let chunks: Vec<Vec<PathBuf>> = files_to_process
        .chunks(chunk_size)
        .map(|chunk| chunk.to_vec())
        .collect();

    let (tx, rx) = std::sync::mpsc::channel();
    
    let handles: Vec<_> = chunks.into_iter().map(|chunk| {
        let config = config.clone();
        let partition = partition_name.clone();
        let existing = existing_contexts.clone();
        let progress = progress_tracker.clone();
        let tx = tx.clone();

        thread::spawn(move || -> Result<()> { 
            let mut results = Vec::new();
            process_chunk(chunk, &config, &partition, &existing, progress, &mut results)?;
            tx.send(results).map_err(|e| anyhow::anyhow!("Channel send error: {}", e))?;
            Ok(())
        })
    }).collect();

    for handle in handles {
        handle.join().unwrap()?;
    }

    drop(tx);

    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(&config.file_contexts)?;

    for result_batch in rx {
        for line in result_batch {
            writeln!(file, "{}", line)?;
        }
    }
    progress_tracker.finish();
    if !config.silent && missing_count > 0 {
        println!();
    }

    Ok(())
}

fn count_missing_entries(
    files_to_process: &[PathBuf],
    config: &Config,
    partition: &str,
    existing_contexts: &HashSet<String>,
) -> Result<usize> {
    let mut missing_count = 0;

    for relative_path in files_to_process {
        if let Some(path_str) = relative_path.to_str() {
            if path_str.is_empty() {
                continue;
            }
            
            let escaped_path = escape_regex(path_str);
            let full_context_path = format!("/{}/{} ", partition, escaped_path);
            let folder_context_path = format!("/{}/{}{} ", partition, escaped_path, config.fstype.folder_pattern());
            
            let full_context_trimmed = full_context_path.trim().to_string();
            let folder_context_trimmed = folder_context_path.trim().to_string();
            
            if !existing_contexts.contains(&full_context_trimmed) &&
               !existing_contexts.contains(&folder_context_trimmed) {
                missing_count += 1;
            }
        }
    }
    
    Ok(missing_count)
}

fn get_partition_name(extracted_dir: &PathBuf) -> String {
    extracted_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn read_existing_contexts(path: &PathBuf) -> Result<HashSet<String>> {
    let mut contexts = HashSet::new();
    
    if let Ok(file) = File::open(path) {
        for line in BufReader::new(file).lines() {
            if let Ok(line) = line {
                if let Some(path_part) = line.split_whitespace().next() {
                    contexts.insert(path_part.to_string());
                }
            }
        }
    }
    Ok(contexts)
}

fn collect_files_to_process(config: &Config) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    
    for entry in WalkDir::new(&config.extracted_dir)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        
        if let Ok(relative_path) = path.strip_prefix(&config.extracted_dir) {
            if relative_path.as_os_str().is_empty() {
                continue;
            }
            match config.mode {
                crate::config::Mode::Bin => {
                    if let Some(path_str) = relative_path.to_str() {
                        if path_str.contains("/bin/") {
                            files.push(relative_path.to_path_buf());
                        }
                    }
                }
                crate::config::Mode::All => {
                    files.push(relative_path.to_path_buf());
                }
            }
        }
    }
    Ok(files)
}

fn process_chunk(
    chunk: Vec<PathBuf>,
    config: &Config,
    partition: &str,
    existing_contexts: &HashSet<String>,
    progress: ProgressTracker,
    results: &mut Vec<String>,
) -> Result<()> {
    for relative_path in chunk {
        let full_path = config.extracted_dir.join(&relative_path);
        
        if let Some(path_str) = relative_path.to_str() {
            if path_str.is_empty() {
                progress.increment();
                continue;
            }
            
            let escaped_path = escape_regex(path_str);
            let full_context_path = format!("/{}/{} ", partition, escaped_path);
            let folder_context_path = format!("/{}/{}{} ", partition, escaped_path, config.fstype.folder_pattern());
            
            let full_context_trimmed = full_context_path.trim().to_string();
            let folder_context_trimmed = folder_context_path.trim().to_string();
            
            if !existing_contexts.contains(&full_context_trimmed) &&
               !existing_contexts.contains(&folder_context_trimmed) {
                let is_file = full_path.is_file();

                if is_file {
                    let context_line = process_files(&escaped_path, partition)?;
                    results.push(context_line);
                } else {
                    let context_line = process_dirs(&escaped_path, partition, &config.fstype)?;
                    results.push(context_line);
                }
            }
        }
        progress.increment();
    }
    Ok(())
}

fn process_files(
    escaped_path: &str,
    partition: &str,
) -> Result<String> {
    let processed_path = format!("/{}", escaped_path);
    
    let context = if processed_path.contains("/bin/hw/") {
        "u:object_r:hal_allocator_default_exec:s0"
    } else if processed_path.contains("/bin/") {
        if !partition.contains("vendor") && !partition.contains("odm") {
            "u:object_r:system_file:s0"
        } else {
            "u:object_r:vendor_qti_init_shell_exec:s0"
        }
    } else if !partition.contains("vendor") && !partition.contains("odm") &&
              (processed_path.contains("/lib/") || processed_path.contains("/lib64/")) {
        "u:object_r:system_lib_file:s0"
    } else if partition.contains("vendor") || partition.contains("odm") {
        if processed_path.contains("/etc/") {
            "u:object_r:vendor_configs_file:s0"
        } else if processed_path.contains("/firmware/") {
            "u:object_r:vendor_firmware_file:s0"
        } else if processed_path.contains("/app/") || processed_path.contains("/priv-app/") {
            "u:object_r:vendor_app_file:s0"
        } else if processed_path.contains("/framework/") {
            "u:object_r:vendor_framework_file:s0"
        } else if processed_path.contains("/overlay/") {
            "u:object_r:vendor_overlay_file:s0"
        } else {
            "u:object_r:vendor_file:s0"
        }
    } else {
        "u:object_r:system_file:s0"
    };

    Ok(format!("/{}/{} {}", partition, escaped_path, context))
}

fn process_dirs(
    escaped_path: &str,
    partition: &str,
    fstype: &crate::config::FilesystemType,
) -> Result<String> {
    
    let processed_path = format!("/{}", escaped_path);
    
    let context = if partition.contains("vendor") || partition.contains("odm") {
        if processed_path.contains("/etc") {
            "u:object_r:vendor_configs_file:s0"
        } else if processed_path.contains("/firmware") {
            "u:object_r:vendor_firmware_file:s0"
        } else if processed_path.contains("/app") || processed_path.contains("/priv-app") {
            "u:object_r:vendor_app_file:s0"
        } else if processed_path.contains("/framework") {
            "u:object_r:vendor_framework_file:s0"
        } else if processed_path.contains("/overlay") {
            "u:object_r:vendor_overlay_file:s0"
        } else {
            "u:object_r:vendor_file:s0"
        }
    } else {
        "u:object_r:system_file:s0"
    };

    Ok(format!("/{}/{}{} {}", partition, escaped_path, fstype.folder_pattern(), context))
}