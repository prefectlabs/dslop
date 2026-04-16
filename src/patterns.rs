/// Pattern definitions for detecting AI writing slop.

pub struct Pattern {
    pub name: &'static str,
    pub fix: &'static str,
    pub detect: fn(&str) -> Vec<Match>,
}

pub struct Match {
    pub line_number: usize,
    pub column: usize,
}

// ---- Em-dash ----

fn detect_em_dash(contents: &str) -> Vec<Match> {
    let mut matches = Vec::new();
    for (line_idx, line) in contents.lines().enumerate() {
        for (byte_offset, _) in line.match_indices('\u{2014}') {
            let column = line[..byte_offset].chars().count() + 1;
            matches.push(Match {
                line_number: line_idx + 1,
                column,
            });
        }
    }
    matches
}

pub const EM_DASH: Pattern = Pattern {
    name: "em-dash",
    fix: "rewrite without em-dash (\u{2014}); use a comma, semicolon, or split into two sentences",
    detect: detect_em_dash,
};

// ---- Double-hyphen (em-dash substitute) ----

fn detect_double_hyphen(contents: &str) -> Vec<Match> {
    let mut matches = Vec::new();
    for (line_idx, line) in contents.lines().enumerate() {
        // Match " -- " (space-surrounded double hyphen used as dash punctuation).
        // Avoid matching triple-hyphens (---) which are markdown horizontal rules
        // and avoid matching inside code fences or CLI flags like --verbose.
        let bytes = line.as_bytes();
        for (i, _) in line.match_indices("--") {
            // Must not be part of a longer run of hyphens (--- or more)
            let before = if i > 0 { bytes[i - 1] } else { b' ' };
            let after_end = i + 2;
            let after = if after_end < bytes.len() { bytes[after_end] } else { b' ' };
            if before == b'-' || after == b'-' {
                continue;
            }
            // Must have a space (or start-of-line) before and space (or end-of-line) after
            if before != b' ' || after != b' ' {
                continue;
            }
            let column = line[..i].chars().count() + 1;
            matches.push(Match {
                line_number: line_idx + 1,
                column,
            });
        }
    }
    matches
}

pub const DOUBLE_HYPHEN: Pattern = Pattern {
    name: "double-hyphen",
    fix: "rewrite without \" -- \"; use a comma, semicolon, or split into two sentences",
    detect: detect_double_hyphen,
};

// ---- Contrastive parallelism ----

fn detect_contrastive(contents: &str) -> Vec<Match> {
    // Lazy-init the regex once per call is fine; for hot paths we'd use LazyLock
    // but pattern checks are I/O-bound anyway.
    let re = regex_lite::Regex::new(
        r"(?i)\b(it'?s not|it isn'?t|this isn'?t|this is not|that'?s not|that isn'?t)\s+(just|merely|simply\s+)?.{1,50}[,;\u{2014}]\s*(it'?s|it is|this is|that'?s)\b"
    ).unwrap();

    let mut matches = Vec::new();
    let mut line_start = 0;
    for (line_idx, line) in contents.lines().enumerate() {
        for m in re.find_iter(line) {
            let column = line[..m.start()].chars().count() + 1;
            matches.push(Match {
                line_number: line_idx + 1,
                column,
            });
        }
        line_start += line.len() + 1;
    }
    let _ = line_start; // suppress unused warning

    matches
}

pub const CONTRASTIVE: Pattern = Pattern {
    name: "contrastive",
    fix: "rephrase to avoid \"it's not X, it's Y\" construction; state what it *is* directly",
    detect: detect_contrastive,
};

use crate::config::Config;

pub fn active_patterns(config: &Config) -> Vec<&'static Pattern> {
    let mut out = Vec::new();
    if config.patterns.em_dash {
        out.push(&EM_DASH);
    }
    if config.patterns.double_hyphen {
        out.push(&DOUBLE_HYPHEN);
    }
    if config.patterns.contrastive {
        out.push(&CONTRASTIVE);
    }
    out
}
