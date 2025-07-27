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
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let config_path = cwd.join("config.yaml");
    let config: Config = if config_path.exists() {
        let config_str = fs::read_to_string(&config_path).context("Failed to read config.yaml")?;
        serde_yaml::from_str(&config_str).context("Failed to parse config.yaml")?
    } else {
        println!("No config.yaml found, using defaults.");
        Config::default()
    };

    let exclude_dir_set: Vec<_> = config.exclude_dirs.iter().map(|s| s.as_str()).collect();
    let exclude_file_set: Vec<_> = config.exclude_files.iter().map(|s| s.as_str()).collect();

    let mut glob_builder = GlobSetBuilder::new();
    for pat in &config.exclude_patterns {
        glob_builder.add(Glob::new(pat).context(format!("Invalid glob pattern: {}", pat))?);
    }
    let exclude_pattern_set = glob_builder.build().context("Failed to build globset")?;

    let output_path = cwd.join("bundle.txt");
    let mut output_file = File::create(&output_path).context("Failed to create bundle.txt")?;

    for entry in WalkDir::new(&cwd).into_iter().filter_map(|e| e.ok()) {
        if should_skip(&entry, &cwd, &exclude_dir_set, &exclude_file_set, &exclude_pattern_set) {
            continue;
        }

        if entry.file_type().is_file() {
            if let Err(e) = process_file(&entry.path(), &cwd, &mut output_file) {
                eprintln!("Warning: Failed to process {}: {}", entry.path().display(), e);
            }
        }
    }

    println!("Bundle created at: {}", output_path.display());
    Ok(())
}

fn should_skip(
    entry: &DirEntry,
    cwd: &Path,
    exclude_dirs: &[&str],
    exclude_files: &[&str],
    exclude_patterns: &GlobSet,
) -> bool {
    let rel_path = entry.path().strip_prefix(cwd).unwrap_or(entry.path());
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

fn process_file(path: &Path, cwd: &Path, output: &mut File) -> Result<()> {
    let rel_path = path.strip_prefix(cwd).unwrap_or(path).display();

    // Check if text (try reading as UTF-8)
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
