use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};
use serde::Deserialize;
use globset::{Glob, GlobSet, GlobSetBuilder};
use anyhow::{Context, Result};

#[derive(Deserialize, Default)]
struct Config {
    exclude_dirs: Vec<String>,
    exclude_files: Vec<String>,
    exclude_patterns: Vec<String>,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        return Err(anyhow::anyhow!("Usage: {} <input_dir> <output_file> <config_path>", args[0]));
    }

    let input_dir = PathBuf::from(&args[1]);
    if !input_dir.exists() || !input_dir.is_dir() {
        return Err(anyhow::anyhow!("Input path must be an existing directory: {}", input_dir.display()));
    }

    let output_path = PathBuf::from(&args[2]);

    let config_path = PathBuf::from(&args[3]);
    let config: Config = if config_path.exists() {
        let config_str = fs::read_to_string(&config_path).context("Failed to read config file")?;
        serde_yaml::from_str(&config_str).context("Failed to parse config file")?
    } else {
        println!("No config file found at {}, using defaults.", config_path.display());
        Config::default()
    };

    let exclude_dir_set: Vec<_> = config.exclude_dirs.iter().map(|s| s.as_str()).collect();
    let exclude_file_set: Vec<_> = config.exclude_files.iter().map(|s| s.as_str()).collect();

    let mut glob_builder = GlobSetBuilder::new();
    for pat in &config.exclude_patterns {
        glob_builder.add(Glob::new(pat).context(format!("Invalid glob pattern: {}", pat))?);
    }
    let exclude_pattern_set = glob_builder.build().context("Failed to build globset")?;

    let mut output_file = File::create(&output_path).context("Failed to create output file")?;

    for entry in WalkDir::new(&input_dir).into_iter().filter_map(|e| e.ok()) {
        if should_skip(&entry, &input_dir, &exclude_dir_set, &exclude_file_set, &exclude_pattern_set) {
            continue;
        }

        if entry.file_type().is_file() {
            if let Err(e) = process_file(&entry.path(), &input_dir, &mut output_file) {
                eprintln!("Warning: Failed to process {}: {}", entry.path().display(), e);
            }
        }
    }

    println!("Bundle created at: {}", output_path.display());
    Ok(())
}

fn should_skip(
    entry: &DirEntry,
    root: &Path,
    exclude_dirs: &[&str],
    exclude_files: &[&str],
    exclude_patterns: &GlobSet,
) -> bool {
    let rel_path = entry.path().strip_prefix(root).unwrap_or(entry.path());
    let rel_str = rel_path.to_string_lossy();

    if entry.file_type().is_dir() {
        exclude_dirs.iter().any(|&dir| rel_str == dir)
    } else {
        let is_in_excluded_dir = exclude_dirs.iter().any(|&dir| {
            let prefix = format!("{}/", dir);
            rel_str.starts_with(&prefix)
        });
        is_in_excluded_dir ||
        exclude_files.contains(&rel_str.as_ref()) ||
        exclude_patterns.is_match(rel_path)
    }
}

fn process_file(path: &Path, root: &Path, output: &mut File) -> Result<()> {
    let rel_path = path.strip_prefix(root).unwrap_or(path).display();

    let file = File::open(path).context("Failed to open file")?;
    let reader = BufReader::new(file);
    let mut content = String::new();
    for line in reader.lines() {
        content.push_str(&line.context("Failed to read line")?);
        content.push('\n');
    }

    writeln!(output, "--- START FILE: {} ---", rel_path)?;
    output.write_all(content.as_bytes())?;
    writeln!(output, "--- END FILE ---\n")?;

    Ok(())
}
