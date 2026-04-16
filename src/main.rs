mod check;
mod config;
mod metrics;
mod output;
mod patterns;

use std::path::Path;
use std::process;

use clap::Parser;

#[derive(Parser)]
#[command(name = "dslop", about = "Detect AI writing patterns (slop) in your codebase")]
struct Cli {
    /// Files or directories to check (defaults to current directory)
    paths: Vec<String>,

    /// Path to config file (default: search upward for dslop.toml)
    #[arg(short, long)]
    config: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    let start = cli
        .paths
        .first()
        .map(|p| Path::new(p.as_str()))
        .unwrap_or(Path::new("."));

    let config = if let Some(ref path) = cli.config {
        config::Config::load_from(Path::new(path))
    } else {
        config::Config::load(start)
    };

    let paths: Vec<&Path> = if cli.paths.is_empty() {
        vec![Path::new(".")]
    } else {
        cli.paths.iter().map(|p| Path::new(p.as_str())).collect()
    };

    let active_patterns = patterns::active_patterns(&config);
    let results = check::check_paths(&paths, &active_patterns, &config);

    if results.is_empty() {
        process::exit(0);
    }

    output::print_results(&results);
    process::exit(1);
}
