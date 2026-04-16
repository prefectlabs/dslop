use std::fs;
use std::path::Path;

use crate::config::Config;
use crate::metrics;
use crate::patterns::{Match, Pattern};

pub struct FileResult {
    pub path: String,
    pub findings: Vec<Finding>,
    pub metric_violations: Vec<MetricViolation>,
}

pub struct Finding {
    pub pattern_name: &'static str,
    pub fix: &'static str,
    pub matches: Vec<Match>,
}

pub struct MetricViolation {
    pub metric_name: &'static str,
    pub fix: &'static str,
    pub score: f64,
    pub threshold: f64,
}

pub fn check_file(path: &Path, patterns: &[&Pattern], config: &Config, run_metrics: bool) -> Option<FileResult> {
    let contents = fs::read_to_string(path).ok()?;
    check_contents(&contents, &path.display().to_string(), patterns, config, run_metrics)
}

/// Run pattern and metric checks against an in-memory string.
/// Used for stdin and anywhere else we don't want to touch the filesystem.
pub fn check_contents(
    contents: &str,
    path_label: &str,
    patterns: &[&Pattern],
    config: &Config,
    run_metrics: bool,
) -> Option<FileResult> {
    let mut findings = Vec::new();

    for pattern in patterns {
        let matches = (pattern.detect)(contents);
        if !matches.is_empty() {
            findings.push(Finding {
                pattern_name: pattern.name,
                fix: pattern.fix,
                matches,
            });
        }
    }

    let mut metric_violations = Vec::new();

    if run_metrics {
        if let Some(thresh) = config.metrics.sentence_length_cv {
            if let Some(score) = metrics::sentence_length_cv(contents) {
                if score < thresh {
                    metric_violations.push(MetricViolation {
                        metric_name: "sentence-length-cv",
                        fix: "writing has a metronomic rhythm — every sentence hits the same beat. vary structure: combine related ideas, break apart compound ones, use subordinate clauses or fragments where natural",
                        score,
                        threshold: thresh,
                    });
                }
            }
        }

        if let Some(thresh) = config.metrics.sentence_length_kurtosis {
            if let Some(score) = metrics::sentence_length_kurtosis(contents) {
                if score < thresh {
                    metric_violations.push(MetricViolation {
                        metric_name: "sentence-length-kurtosis",
                        fix: "your sentences are all similar length. rewrite whole paragraphs: merge related short sentences into longer, layered ones; also insert a few very short or very long sentences for contrast",
                        score,
                        threshold: thresh,
                    });
                }
            }
        }

        if let Some(thresh) = config.metrics.word_freq_dispersion.threshold {
            let chunk_size = config.metrics.word_freq_dispersion.chunk_size;
            let top_n = config.metrics.word_freq_dispersion.top_n;
            if let Some(score) = metrics::word_freq_dispersion(contents, chunk_size, top_n) {
                if score < thresh {
                    metric_violations.push(MetricViolation {
                        metric_name: "word-freq-dispersion",
                        fix: "key words are sprinkled too uniformly — concentrate terminology where it's relevant instead of distributing it evenly across the text",
                        score,
                        threshold: thresh,
                    });
                }
            }
        }
    }

    let has_violations = !findings.is_empty() || !metric_violations.is_empty();

    if has_violations {
        Some(FileResult {
            path: path_label.to_string(),
            findings,
            metric_violations,
        })
    } else {
        None
    }
}

pub fn check_paths(paths: &[&Path], patterns: &[&Pattern], config: &Config) -> Vec<FileResult> {
    let mut results = Vec::new();
    for path in paths {
        if path.is_dir() {
            let walker = ignore::WalkBuilder::new(path).hidden(true).build();
            for entry in walker.flatten() {
                if entry.file_type().is_some_and(|ft| ft.is_file()) {
                    let run_metrics = should_run_metrics(entry.path(), config);
                    if let Some(result) = check_file(entry.path(), patterns, config, run_metrics) {
                        results.push(result);
                    }
                }
            }
        } else if path.is_file() {
            let run_metrics = should_run_metrics(path, config);
            if let Some(result) = check_file(path, patterns, config, run_metrics) {
                results.push(result);
            }
        }
    }
    results
}

/// Only run metrics on prose-like file extensions.
fn should_run_metrics(path: &Path, config: &Config) -> bool {
    if config.metrics.extensions.is_empty() {
        return true;
    }
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| config.metrics.extensions.iter().any(|e| e == ext))
}
