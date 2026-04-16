mod check;
mod config;
mod metrics;
mod output;
mod patterns;

use std::io::{self, IsTerminal, Read};
use std::path::Path;
use std::process;

use clap::Parser;

#[derive(Parser)]
#[command(name = "dslop", about = "Detect AI writing patterns (slop) in your codebase")]
struct Cli {
    /// Files or directories to check. Pass `-` to read from stdin.
    /// Defaults to the current directory, or stdin when stdin is piped.
    paths: Vec<String>,

    /// Path to config file (default: search upward for dslop.toml)
    #[arg(short, long)]
    config: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    // Decide whether to read from stdin:
    //   - explicit `-` anywhere in the paths
    //   - no paths provided AND stdin is piped (not a TTY)
    let explicit_stdin = cli.paths.iter().any(|p| p == "-");
    let implicit_stdin = cli.paths.is_empty() && !io::stdin().is_terminal();
    let use_stdin = explicit_stdin || implicit_stdin;

    // Config lookup anchors on the first concrete path, or cwd for stdin.
    let config_start = if use_stdin {
        Path::new(".")
    } else {
        cli.paths
            .first()
            .map(|p| Path::new(p.as_str()))
            .unwrap_or(Path::new("."))
    };

    let config = if let Some(ref path) = cli.config {
        config::Config::load_from(Path::new(path))
    } else {
        config::Config::load(config_start)
    };

    let active_patterns = patterns::active_patterns(&config);

    let results = if use_stdin {
        let mut contents = String::new();
        if let Err(e) = io::stdin().read_to_string(&mut contents) {
            eprintln!("dslop: failed to read stdin: {e}");
            process::exit(2);
        }
        // Always run metrics on stdin — there's no extension to filter on,
        // and the user explicitly directed prose here.
        check::check_contents(&contents, "<stdin>", &active_patterns, &config, true)
            .map(|r| vec![r])
            .unwrap_or_default()
    } else {
        let paths: Vec<&Path> = cli.paths.iter().map(|p| Path::new(p.as_str())).collect();
        let paths: Vec<&Path> = if paths.is_empty() {
            vec![Path::new(".")]
        } else {
            paths
        };
        check::check_paths(&paths, &active_patterns, &config)
    };

    if results.is_empty() {
        process::exit(0);
    }

    output::print_results(&results);
    process::exit(1);
}
