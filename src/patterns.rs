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

// ---- Position helpers ----

/// Byte offsets of each line start in `contents` (1-indexed line i at index i-1).
fn line_starts(contents: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    for (i, b) in contents.bytes().enumerate() {
        if b == b'\n' {
            starts.push(i + 1);
        }
    }
    starts
}

fn byte_to_line_col(contents: &str, starts: &[usize], offset: usize) -> (usize, usize) {
    let line_idx = starts.partition_point(|&s| s <= offset).saturating_sub(1);
    let line_start = starts[line_idx];
    let col = contents[line_start..offset.min(contents.len())].chars().count() + 1;
    (line_idx + 1, col)
}

// ---- Sentence and paragraph splitters (naive, prose-targeted) ----

struct Sentence<'a> {
    text: &'a str,
    start: usize,
}

/// Split on terminal punctuation followed by whitespace or EOF. Keeps the
/// terminator in `text`. Ignores structure like markdown headings — good
/// enough for prose linting.
fn sentences(contents: &str) -> Vec<Sentence<'_>> {
    let bytes = contents.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'.' || b == b'!' || b == b'?' {
            let mut j = i + 1;
            while j < bytes.len() && matches!(bytes[j], b'.' | b'!' | b'?') {
                j += 1;
            }
            let after = bytes.get(j).copied();
            if after.is_none() || matches!(after, Some(b' ' | b'\n' | b'\t' | b'\r')) {
                let text = &contents[start..j];
                if !text.trim().is_empty() {
                    out.push(Sentence { text, start });
                }
                let mut k = j;
                while k < bytes.len() && matches!(bytes[k], b' ' | b'\n' | b'\t' | b'\r') {
                    k += 1;
                }
                start = k;
                i = k;
                continue;
            }
            i = j;
        } else {
            i += 1;
        }
    }
    if start < bytes.len() {
        let text = &contents[start..];
        if !text.trim().is_empty() {
            out.push(Sentence { text, start });
        }
    }
    out
}

struct Paragraph<'a> {
    text: &'a str,
    start: usize,
}

fn paragraphs(contents: &str) -> Vec<Paragraph<'_>> {
    let re = regex_lite::Regex::new(r"\n[ \t]*\n").unwrap();
    let mut out = Vec::new();
    let mut start = 0usize;
    for m in re.find_iter(contents) {
        let text = &contents[start..m.start()];
        if !text.trim().is_empty() {
            out.push(Paragraph { text, start });
        }
        start = m.end();
    }
    let text = &contents[start..];
    if !text.trim().is_empty() {
        out.push(Paragraph { text, start });
    }
    out
}

fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
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

// ---- En-dash ----

fn detect_en_dash(contents: &str) -> Vec<Match> {
    let mut matches = Vec::new();
    for (line_idx, line) in contents.lines().enumerate() {
        for (byte_offset, _) in line.match_indices('\u{2013}') {
            let column = line[..byte_offset].chars().count() + 1;
            matches.push(Match {
                line_number: line_idx + 1,
                column,
            });
        }
    }
    matches
}

pub const EN_DASH: Pattern = Pattern {
    name: "en-dash",
    fix: "rewrite without en-dash (\u{2013}); write ranges with \"to\" and use plain punctuation elsewhere",
    detect: detect_en_dash,
};

// ---- Double-hyphen (em-dash substitute) ----

fn detect_double_hyphen(contents: &str) -> Vec<Match> {
    let mut matches = Vec::new();
    for (line_idx, line) in contents.lines().enumerate() {
        let bytes = line.as_bytes();
        for (i, _) in line.match_indices("--") {
            let before = if i > 0 { bytes[i - 1] } else { b' ' };
            let after_end = i + 2;
            let after = if after_end < bytes.len() { bytes[after_end] } else { b' ' };
            if before == b'-' || after == b'-' {
                continue;
            }
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
    let re = regex_lite::Regex::new(
        r"(?i)\b(it'?s not|it isn'?t|this isn'?t|this is not|that'?s not|that isn'?t)\s+(just|merely|simply\s+)?.{1,50}(?:[,;\u{2014}]|--)\s*(it'?s|it is|this is|that'?s)\b"
    ).unwrap();

    let mut matches = Vec::new();
    for (line_idx, line) in contents.lines().enumerate() {
        for m in re.find_iter(line) {
            let column = line[..m.start()].chars().count() + 1;
            matches.push(Match {
                line_number: line_idx + 1,
                column,
            });
        }
    }
    matches
}

pub const CONTRASTIVE: Pattern = Pattern {
    name: "contrastive",
    fix: "rephrase to avoid \"it's not X, it's Y\" construction; state what it *is* directly",
    detect: detect_contrastive,
};

// ---- Demonstrative-is ("This is the X", "That is the X") ----

fn detect_demonstrative_is(contents: &str) -> Vec<Match> {
    let re = regex_lite::Regex::new(r"(?i)\b(?:this|that)\s+is\s+the\b").unwrap();
    let mut matches = Vec::new();
    for (line_idx, line) in contents.lines().enumerate() {
        for m in re.find_iter(line) {
            let column = line[..m.start()].chars().count() + 1;
            matches.push(Match {
                line_number: line_idx + 1,
                column,
            });
        }
    }
    matches
}

pub const DEMONSTRATIVE_IS: Pattern = Pattern {
    name: "demonstrative-is",
    fix: "rewrite without \"this is the\" / \"that is the\"; name the thing directly or restructure the sentence",
    detect: detect_demonstrative_is,
};

// ---- Filler adverbs ----

fn detect_filler_adverbs(contents: &str) -> Vec<Match> {
    let re = regex_lite::Regex::new(
        r"(?i)\b(quietly|materially|exactly|actually|really|simply|essentially|fundamentally|effectively|arguably)\b"
    ).unwrap();
    let mut matches = Vec::new();
    for (line_idx, line) in contents.lines().enumerate() {
        for m in re.find_iter(line) {
            let column = line[..m.start()].chars().count() + 1;
            matches.push(Match {
                line_number: line_idx + 1,
                column,
            });
        }
    }
    matches
}

pub const FILLER_ADVERBS: Pattern = Pattern {
    name: "filler-adverbs",
    fix: "cut the adverb (quietly/actually/really/simply/essentially/etc.); if it carries weight, replace with a concrete qualifier",
    detect: detect_filler_adverbs,
};

// ---- Weasel connectives ----

fn detect_weasel_connectives(contents: &str) -> Vec<Match> {
    let re = regex_lite::Regex::new(
        r"(?i)\b(which means|in turn|so that|which is to say|it is worth noting|the reality is|at the end of the day)\b"
    ).unwrap();
    let mut matches = Vec::new();
    for (line_idx, line) in contents.lines().enumerate() {
        for m in re.find_iter(line) {
            let column = line[..m.start()].chars().count() + 1;
            matches.push(Match {
                line_number: line_idx + 1,
                column,
            });
        }
    }
    matches
}

pub const WEASEL_CONNECTIVES: Pattern = Pattern {
    name: "weasel-connectives",
    fix: "drop the connective (\"which means\", \"in turn\", \"the reality is\", etc.) and state the consequence directly",
    detect: detect_weasel_connectives,
};

// ---- Banned flourish phrases ----

fn detect_banned_flourish(contents: &str) -> Vec<Match> {
    let re = regex_lite::Regex::new(
        r"(?i)\b(worth noting|in the end|at the end of the day|to be clear|let me be specific|needless to say|fails to translate|structurally can(?:not|'t)|in a meaningful way|in any real sense)\b"
    ).unwrap();
    let mut matches = Vec::new();
    for (line_idx, line) in contents.lines().enumerate() {
        for m in re.find_iter(line) {
            let column = line[..m.start()].chars().count() + 1;
            matches.push(Match {
                line_number: line_idx + 1,
                column,
            });
        }
    }
    matches
}

pub const BANNED_FLOURISH: Pattern = Pattern {
    name: "banned-flourish",
    fix: "delete the flourish phrase (\"worth noting\", \"to be clear\", \"at the end of the day\", etc.); the sentence is stronger without it",
    detect: detect_banned_flourish,
};

// ---- Stock AI vocabulary ----

fn detect_ai_vocabulary(contents: &str) -> Vec<Match> {
    let re = regex_lite::Regex::new(
        r"(?i)\b(delve|crucial|pivotal|intricate|tapestry|underscore|testament|foster|vibrant|showcase|garner|seamless|robust|leverage|unlock|empower|elevate|transform|revolutionize|synergy|game-changer|landscape|reach for)\b"
    ).unwrap();
    let mut matches = Vec::new();
    for (line_idx, line) in contents.lines().enumerate() {
        for m in re.find_iter(line) {
            let column = line[..m.start()].chars().count() + 1;
            matches.push(Match {
                line_number: line_idx + 1,
                column,
            });
        }
    }
    matches
}

pub const AI_VOCABULARY: Pattern = Pattern {
    name: "ai-vocabulary",
    fix: "replace stock AI vocabulary with an ordinary word or the specific fact",
    detect: detect_ai_vocabulary,
};

// ---- Consecutive rhetorical questions ----

fn detect_rhetorical_question_chain(contents: &str) -> Vec<Match> {
    let starts = line_starts(contents);
    let mut matches = Vec::new();
    for p in paragraphs(contents) {
        let mut first_question: Option<usize> = None;
        let mut run_len = 0usize;
        for s in sentences(p.text) {
            if s.text.trim_end().ends_with('?') {
                first_question.get_or_insert(s.start);
                run_len += 1;
                if run_len == 2 {
                    let (line, col) =
                        byte_to_line_col(contents, &starts, p.start + first_question.unwrap());
                    matches.push(Match {
                        line_number: line,
                        column: col,
                    });
                }
            } else {
                first_question = None;
                run_len = 0;
            }
        }
    }
    matches
}

pub const RHETORICAL_QUESTION_CHAIN: Pattern = Pattern {
    name: "rhetorical-question-chain",
    fix: "replace consecutive rhetorical questions with a direct statement of the problem",
    detect: detect_rhetorical_question_chain,
};

// ---- Vague attribution ----

fn detect_vague_attribution(contents: &str) -> Vec<Match> {
    let re = regex_lite::Regex::new(
        r"(?i)\b(experts (?:say|argue|believe)|studies show|research shows|observers (?:say|argue|have cited)|many (?:say|argue|believe)|it is widely believed)\b"
    ).unwrap();
    let mut matches = Vec::new();
    for (line_idx, line) in contents.lines().enumerate() {
        for m in re.find_iter(line) {
            let column = line[..m.start()].chars().count() + 1;
            matches.push(Match {
                line_number: line_idx + 1,
                column,
            });
        }
    }
    matches
}

pub const VAGUE_ATTRIBUTION: Pattern = Pattern {
    name: "vague-attribution",
    fix: "name the source and evidence behind the claim, or cut the attribution",
    detect: detect_vague_attribution,
};

// ---- Count-opening ----

fn detect_count_opening(contents: &str) -> Vec<Match> {
    let starts = line_starts(contents);
    let re = regex_lite::Regex::new(
        r"(?i)^\s*(?:there (?:are|is)\s+)?(?:a few|two|three|four|five|six|seven|eight|nine|ten)\s+(?:things|reasons|points|notes|cautions|ways|steps|takeaways|considerations)\b"
    ).unwrap();
    let mut matches = Vec::new();
    for s in sentences(contents) {
        if let Some(m) = re.find(s.text) {
            let (line, col) = byte_to_line_col(contents, &starts, s.start + m.start());
            matches.push(Match {
                line_number: line,
                column: col,
            });
        }
    }
    matches
}

pub const COUNT_OPENING: Pattern = Pattern {
    name: "count-opening",
    fix: "state the first point directly; use a list when the count helps the reader",
    detect: detect_count_opening,
};

// ---- Banned negation ("not" / "n't") ----

fn detect_banned_negation(contents: &str) -> Vec<Match> {
    let re = regex_lite::Regex::new(r"(?i)(\bnot\b|\b[[:alpha:]]+n['\u{2019}]t\b)").unwrap();
    let mut matches = Vec::new();
    for (line_idx, line) in contents.lines().enumerate() {
        for m in re.find_iter(line) {
            let column = line[..m.start()].chars().count() + 1;
            matches.push(Match {
                line_number: line_idx + 1,
                column,
            });
        }
    }
    matches
}

pub const BANNED_NEGATION: Pattern = Pattern {
    name: "banned-negation",
    fix: "rewrite without \"not\" or \"n't\"; state the positive claim directly",
    detect: detect_banned_negation,
};

// ---- Negation pair ("not X, not Y" / "not X. not Y.") ----

fn detect_negation_pair(contents: &str) -> Vec<Match> {
    let re = regex_lite::Regex::new(
        r"(?i)\bnot\s+\w+\s*[,.]\s+(?:not|no)\s+\w+"
    ).unwrap();
    let mut matches = Vec::new();
    for (line_idx, line) in contents.lines().enumerate() {
        for m in re.find_iter(line) {
            let column = line[..m.start()].chars().count() + 1;
            matches.push(Match {
                line_number: line_idx + 1,
                column,
            });
        }
    }
    matches
}

pub const NEGATION_PAIR: Pattern = Pattern {
    name: "negation-pair",
    fix: "stop listing what the thing isn't; say what it is",
    detect: detect_negation_pair,
};

// ---- Symmetric X/not-X ----

fn detect_symmetric_negation(contents: &str) -> Vec<Match> {
    // "fine if ... wrong if", "not ... but", "X, or nothing., Y"
    let re = regex_lite::Regex::new(
        r"(?i)(\bfine if\b.{0,40}\bwrong if\b|\bnot\b[^,.;\n]{0,30}\bbut\b|,\s*or nothing\.)"
    ).unwrap();
    let mut matches = Vec::new();
    for (line_idx, line) in contents.lines().enumerate() {
        for m in re.find_iter(line) {
            let column = line[..m.start()].chars().count() + 1;
            matches.push(Match {
                line_number: line_idx + 1,
                column,
            });
        }
    }
    matches
}

pub const SYMMETRIC_NEGATION: Pattern = Pattern {
    name: "symmetric-negation",
    fix: "avoid mirrored \"X if ... Y if ...\" or \"not X but Y\" scaffolding; commit to a single framing",
    detect: detect_symmetric_negation,
};

// ---- Which-chain (3+ "which" in one sentence) ----

fn detect_which_chain(contents: &str) -> Vec<Match> {
    let starts = line_starts(contents);
    let re = regex_lite::Regex::new(r"(?i)\bwhich\b").unwrap();
    let mut matches = Vec::new();
    for s in sentences(contents) {
        let count = re.find_iter(s.text).count();
        if count >= 3 {
            let (line, col) = byte_to_line_col(contents, &starts, s.start);
            matches.push(Match {
                line_number: line,
                column: col,
            });
        }
    }
    matches
}

pub const WHICH_CHAIN: Pattern = Pattern {
    name: "which-chain",
    fix: "break the sentence apart; 3+ \"which\" clauses in one sentence is a run-on tell",
    detect: detect_which_chain,
};

// ---- But-opener (sentence starts with "But", <= 8 words) ----

fn detect_but_opener(contents: &str) -> Vec<Match> {
    let starts = line_starts(contents);
    let mut matches = Vec::new();
    for s in sentences(contents) {
        let trimmed = s.text.trim_start();
        let leading_ws = s.text.len() - trimmed.len();
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("but ") && word_count(trimmed) <= 8 {
            let (line, col) = byte_to_line_col(contents, &starts, s.start + leading_ws);
            matches.push(Match {
                line_number: line,
                column: col,
            });
        }
    }
    matches
}

pub const BUT_OPENER: Pattern = Pattern {
    name: "but-opener",
    fix: "don't open a short sentence with \"But\" as a rhetorical pivot; merge with the prior clause or drop it",
    detect: detect_but_opener,
};

// ---- Aphorism ("X does not VERB.") ----

fn detect_aphorism(contents: &str) -> Vec<Match> {
    let starts = line_starts(contents);
    // Single-clause short sentence of the form "<word> does/do/doesn't/don't [not] <verb>."
    let re = regex_lite::Regex::new(
        r"(?i)^\s*\S+\s+(?:does|do|doesn'?t|don'?t)(?:\s+not)?\s+\w+\s*[.!?]+\s*$"
    ).unwrap();
    let mut matches = Vec::new();
    for s in sentences(contents) {
        if word_count(s.text) <= 6 && re.is_match(s.text) {
            let trimmed = s.text.trim_start();
            let leading_ws = s.text.len() - trimmed.len();
            let (line, col) = byte_to_line_col(contents, &starts, s.start + leading_ws);
            matches.push(Match {
                line_number: line,
                column: col,
            });
        }
    }
    matches
}

pub const APHORISM: Pattern = Pattern {
    name: "aphorism",
    fix: "drop the short \"X does not Y\" aphorism; make the concrete point it's gesturing at",
    detect: detect_aphorism,
};

// ---- Summary capstone (paragraph-final "This/That/It/Here is the X.") ----

fn detect_summary_capstone(contents: &str) -> Vec<Match> {
    let starts = line_starts(contents);
    let re = regex_lite::Regex::new(
        r"(?i)^\s*(?:that|this|it|here)(?:'s| is| was| will be)(?:\s+(?:the|a|our))?\s+\w{1,12}\s*[.!?]+\s*$"
    ).unwrap();
    let mut matches = Vec::new();
    for p in paragraphs(contents) {
        let sents = sentences(p.text);
        let Some(last) = sents.last() else { continue };
        if re.is_match(last.text) {
            let abs_start = p.start + last.start;
            let trimmed = last.text.trim_start();
            let leading_ws = last.text.len() - trimmed.len();
            let (line, col) = byte_to_line_col(contents, &starts, abs_start + leading_ws);
            matches.push(Match {
                line_number: line,
                column: col,
            });
        }
    }
    matches
}

pub const SUMMARY_CAPSTONE: Pattern = Pattern {
    name: "summary-capstone",
    fix: "cut the paragraph-ending capstone (\"That is the X.\", \"Here is the Y.\"); let the prior sentences carry the point",
    detect: detect_summary_capstone,
};

// ---- Declarative-fragment callback ("That is the <noun>.", "<Noun> becomes <noun>.") at paragraph end ----

fn detect_declarative_callback(contents: &str) -> Vec<Match> {
    let starts = line_starts(contents);
    let re_that = regex_lite::Regex::new(
        r"(?i)^\s*(?:that|this) is the \w+\s*\.\s*$"
    ).unwrap();
    let re_becomes = regex_lite::Regex::new(
        r"^\s*[A-Z][a-zA-Z]+ becomes [a-z][a-zA-Z]+\s*\.\s*$"
    ).unwrap();
    let mut matches = Vec::new();
    for p in paragraphs(contents) {
        let sents = sentences(p.text);
        let Some(last) = sents.last() else { continue };
        if re_that.is_match(last.text) || re_becomes.is_match(last.text) {
            let abs_start = p.start + last.start;
            let trimmed = last.text.trim_start();
            let leading_ws = last.text.len() - trimmed.len();
            let (line, col) = byte_to_line_col(contents, &starts, abs_start + leading_ws);
            matches.push(Match {
                line_number: line,
                column: col,
            });
        }
    }
    matches
}

pub const DECLARATIVE_CALLBACK: Pattern = Pattern {
    name: "declarative-callback",
    fix: "drop the declarative callback fragment at the paragraph end; it's rhetorical filler",
    detect: detect_declarative_callback,
};

// ---- Three-beat (3 consecutive <7-word sentences in same paragraph) ----

fn detect_three_beat(contents: &str) -> Vec<Match> {
    let starts = line_starts(contents);
    let mut matches = Vec::new();
    for p in paragraphs(contents) {
        let sents = sentences(p.text);
        let mut run_start: Option<usize> = None;
        let mut run_len = 0usize;
        for s in &sents {
            if word_count(s.text) <= 7 {
                if run_start.is_none() {
                    run_start = Some(s.start);
                }
                run_len += 1;
                if run_len == 3 {
                    let abs = p.start + run_start.unwrap();
                    let (line, col) = byte_to_line_col(contents, &starts, abs);
                    matches.push(Match {
                        line_number: line,
                        column: col,
                    });
                    // reset so a run of 4 doesn't double-flag
                    run_start = None;
                    run_len = 0;
                }
            } else {
                run_start = None;
                run_len = 0;
            }
        }
    }
    matches
}

pub const THREE_BEAT: Pattern = Pattern {
    name: "three-beat",
    fix: "three consecutive short sentences in a row reads as padding; combine two of them or add a longer clause",
    detect: detect_three_beat,
};

use crate::config::Config;

pub fn active_patterns(config: &Config) -> Vec<&'static Pattern> {
    let mut out = Vec::new();
    if config.patterns.em_dash {
        out.push(&EM_DASH);
    }
    if config.patterns.en_dash {
        out.push(&EN_DASH);
    }
    if config.patterns.double_hyphen {
        out.push(&DOUBLE_HYPHEN);
    }
    if config.patterns.contrastive {
        out.push(&CONTRASTIVE);
    }
    if config.patterns.demonstrative_is {
        out.push(&DEMONSTRATIVE_IS);
    }
    if config.patterns.filler_adverbs {
        out.push(&FILLER_ADVERBS);
    }
    if config.patterns.weasel_connectives {
        out.push(&WEASEL_CONNECTIVES);
    }
    if config.patterns.banned_flourish {
        out.push(&BANNED_FLOURISH);
    }
    if config.patterns.ai_vocabulary {
        out.push(&AI_VOCABULARY);
    }
    if config.patterns.rhetorical_question_chain {
        out.push(&RHETORICAL_QUESTION_CHAIN);
    }
    if config.patterns.vague_attribution {
        out.push(&VAGUE_ATTRIBUTION);
    }
    if config.patterns.count_opening {
        out.push(&COUNT_OPENING);
    }
    if config.patterns.banned_negation {
        out.push(&BANNED_NEGATION);
    }
    if config.patterns.negation_pair {
        out.push(&NEGATION_PAIR);
    }
    if config.patterns.symmetric_negation {
        out.push(&SYMMETRIC_NEGATION);
    }
    if config.patterns.which_chain {
        out.push(&WHICH_CHAIN);
    }
    if config.patterns.but_opener {
        out.push(&BUT_OPENER);
    }
    if config.patterns.aphorism {
        out.push(&APHORISM);
    }
    if config.patterns.summary_capstone {
        out.push(&SUMMARY_CAPSTONE);
    }
    if config.patterns.declarative_callback {
        out.push(&DECLARATIVE_CALLBACK);
    }
    if config.patterns.three_beat {
        out.push(&THREE_BEAT);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_plain_punctuation_substitutes() {
        assert_eq!(detect_en_dash("pages 10\u{2013}20").len(), 1);
        assert_eq!(detect_en_dash("pages 10-20").len(), 0);
    }

    #[test]
    fn detects_stock_ai_vocabulary() {
        assert_eq!(detect_ai_vocabulary("Unlock a robust workflow.").len(), 2);
        assert_eq!(detect_ai_vocabulary("Use a faster workflow.").len(), 0);
    }

    #[test]
    fn detects_consecutive_questions_only() {
        assert_eq!(
            detect_rhetorical_question_chain("Does it work? Does it scale?").len(),
            1
        );
        assert_eq!(
            detect_rhetorical_question_chain("Does it work? The benchmark answers that.").len(),
            0
        );
    }

    #[test]
    fn detects_vague_attribution() {
        assert_eq!(detect_vague_attribution("Experts say it scales.").len(), 1);
        assert_eq!(
            detect_vague_attribution("The 2025 benchmark found that it scales.").len(),
            0
        );
    }

    #[test]
    fn detects_count_openings() {
        assert_eq!(detect_count_opening("Three things matter.").len(), 1);
        assert_eq!(
            detect_count_opening("There are two reasons to wait.").len(),
            1
        );
        assert_eq!(detect_count_opening("The first result matters.").len(), 0);
    }
}
