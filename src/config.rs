use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Parsed and resolved configuration with defaults applied.
pub struct Config {
    pub patterns: PatternConfig,
    pub metrics: MetricsConfig,
}

pub struct PatternConfig {
    pub em_dash: bool,
    pub double_hyphen: bool,
    pub contrastive: bool,
}

pub struct MetricsConfig {
    /// None = disabled. Some(threshold) = reject if CV < threshold.
    pub sentence_length_cv: Option<f64>,
    /// None = disabled. Some(threshold) = reject if kurtosis < threshold.
    pub sentence_length_kurtosis: Option<f64>,
    /// Word frequency dispersion config.
    pub word_freq_dispersion: DispersionConfig,
    /// File extensions to run metrics on (empty = all text files).
    pub extensions: Vec<String>,
}

pub struct DispersionConfig {
    /// None = disabled. Some(threshold) = reject if IoD < threshold.
    pub threshold: Option<f64>,
    pub chunk_size: usize,
    pub top_n: usize,
}

// -- TOML shadow types (all optional for merge-onto-defaults) --

#[derive(Deserialize, Default)]
struct RawConfig {
    patterns: Option<RawPatterns>,
    metrics: Option<RawMetrics>,
}

#[derive(Deserialize, Default)]
struct RawPatterns {
    #[serde(rename = "em-dash")]
    em_dash: Option<bool>,
    #[serde(rename = "double-hyphen")]
    double_hyphen: Option<bool>,
    contrastive: Option<bool>,
}

#[derive(Deserialize, Default)]
struct RawMetrics {
    #[serde(rename = "sentence-length-cv")]
    sentence_length_cv: Option<ThresholdOrBool>,
    #[serde(rename = "sentence-length-kurtosis")]
    sentence_length_kurtosis: Option<ThresholdOrBool>,
    #[serde(rename = "word-freq-dispersion")]
    word_freq_dispersion: Option<RawDispersion>,
    extensions: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ThresholdOrBool {
    Threshold(f64),
    Disabled(bool),
}

#[derive(Deserialize, Default)]
struct RawDispersion {
    threshold: Option<f64>,
    enabled: Option<bool>,
    #[serde(rename = "chunk-size")]
    chunk_size: Option<usize>,
    #[serde(rename = "top-n")]
    top_n: Option<usize>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            patterns: PatternConfig {
                em_dash: true,
                double_hyphen: true,
                contrastive: true,
            },
            metrics: MetricsConfig {
                sentence_length_cv: Some(0.3),
                sentence_length_kurtosis: Some(1.5),
                word_freq_dispersion: DispersionConfig {
                    threshold: Some(0.6),
                    chunk_size: 200,
                    top_n: 20,
                },
                extensions: vec![
                    "md".into(),
                    "txt".into(),
                    "rst".into(),
                    "adoc".into(),
                    "tex".into(),
                ],
            },
        }
    }
}

impl Config {
    /// Search upward from `start` for `sf.toml`, parse and merge onto defaults.
    pub fn load(start: &Path) -> Config {
        let mut config = Config::default();

        if let Some(path) = find_config(start) {
            config.load_file(&path);
        }

        config
    }

    /// Load from an explicit config file path.
    pub fn load_from(path: &Path) -> Config {
        let mut config = Config::default();
        config.load_file(path);
        config
    }

    fn load_file(&mut self, path: &Path) {
        if let Ok(contents) = fs::read_to_string(path) {
            if let Ok(raw) = toml::from_str::<RawConfig>(&contents) {
                self.apply(raw);
            }
        }
    }

    fn apply(&mut self, raw: RawConfig) {
        if let Some(p) = raw.patterns {
            if let Some(v) = p.em_dash {
                self.patterns.em_dash = v;
            }
            if let Some(v) = p.double_hyphen {
                self.patterns.double_hyphen = v;
            }
            if let Some(v) = p.contrastive {
                self.patterns.contrastive = v;
            }
        }
        if let Some(m) = raw.metrics {
            if let Some(v) = m.sentence_length_cv {
                self.metrics.sentence_length_cv = resolve_threshold(v);
            }
            if let Some(v) = m.sentence_length_kurtosis {
                self.metrics.sentence_length_kurtosis = resolve_threshold(v);
            }
            if let Some(d) = m.word_freq_dispersion {
                if let Some(false) = d.enabled {
                    self.metrics.word_freq_dispersion.threshold = None;
                } else if let Some(v) = d.threshold {
                    self.metrics.word_freq_dispersion.threshold = Some(v);
                }
                if let Some(v) = d.chunk_size {
                    self.metrics.word_freq_dispersion.chunk_size = v;
                }
                if let Some(v) = d.top_n {
                    self.metrics.word_freq_dispersion.top_n = v;
                }
            }
            if let Some(v) = m.extensions {
                self.metrics.extensions = v;
            }
        }
    }
}

fn resolve_threshold(v: ThresholdOrBool) -> Option<f64> {
    match v {
        ThresholdOrBool::Threshold(t) => Some(t),
        ThresholdOrBool::Disabled(false) => None,
        ThresholdOrBool::Disabled(true) => None, // `true` without a value = keep default, but we've already overwritten; treat as no-op
    }
}

fn find_config(start: &Path) -> Option<PathBuf> {
    let mut dir = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };
    loop {
        let candidate = dir.join("sf.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}
