/// Statistical metrics for detecting AI writing patterns at the file level.
///
/// All functions take `&str` (file contents already in memory) and return a raw score.
/// The caller compares against the configured threshold.

// Minimum sentences/words required for meaningful statistics.
const MIN_SENTENCES: usize = 5;
const MIN_WORDS_FOR_DISPERSION: usize = 100;

// ---- Sentence length CV ----

/// Coefficient of variation of sentence lengths (stdev / mean).
/// Human prose: 0.4–0.7. AI slop: < 0.3.
/// Returns None if too few sentences.
pub fn sentence_length_cv(text: &str) -> Option<f64> {
    let lengths = sentence_word_counts(text);
    if lengths.len() < MIN_SENTENCES {
        return None;
    }
    let mean = mean_f64(&lengths);
    if mean == 0.0 {
        return None;
    }
    let var = variance(&lengths, mean);
    Some(var.sqrt() / mean)
}

// ---- Sentence length kurtosis ----

/// Excess kurtosis of sentence lengths.
/// Low kurtosis = everything samey, no outlier sentences.
/// Returns None if too few sentences.
pub fn sentence_length_kurtosis(text: &str) -> Option<f64> {
    let lengths = sentence_word_counts(text);
    if lengths.len() < MIN_SENTENCES {
        return None;
    }
    let mean = mean_f64(&lengths);
    let m2 = central_moment(&lengths, mean, 2);
    if m2 == 0.0 {
        return None;
    }
    let m4 = central_moment(&lengths, mean, 4);
    Some(m4 / (m2 * m2) - 3.0)
}

// ---- Word frequency index of dispersion ----

/// Average index of dispersion (variance/mean) for top-N word frequencies
/// across fixed-size chunks. Near 1 = Poisson/random, above 1 = bursty (human).
/// Low values = repetitive uniform phrasing (AI).
/// Returns None if text is too short.
pub fn word_freq_dispersion(text: &str, chunk_size: usize, top_n: usize) -> Option<f64> {
    let words = tokenize(text);
    if words.len() < MIN_WORDS_FOR_DISPERSION {
        return None;
    }

    // Global word frequencies (skip stop words).
    let mut freq: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for w in &words {
        if !is_stop_word(w) {
            *freq.entry(w).or_insert(0) += 1;
        }
    }

    // Top-N by frequency.
    let mut top: Vec<(&str, usize)> = freq.into_iter().collect();
    top.sort_unstable_by(|a, b| b.1.cmp(&a.1));
    top.truncate(top_n);

    if top.is_empty() {
        return None;
    }

    // Chunk the words.
    let chunks: Vec<&[&str]> = words.chunks(chunk_size).collect();
    if chunks.len() < 2 {
        return None;
    }

    // For each top word, compute IoD across chunks.
    let mut iod_sum = 0.0;
    let mut iod_count = 0usize;

    for &(word, _) in &top {
        let counts: Vec<f64> = chunks
            .iter()
            .map(|chunk| chunk.iter().filter(|&&w| w == word).count() as f64)
            .collect();

        let mean = mean_f64_raw(&counts);
        if mean == 0.0 {
            continue;
        }
        let var = variance_raw(&counts, mean);
        iod_sum += var / mean;
        iod_count += 1;
    }

    if iod_count == 0 {
        return None;
    }

    Some(iod_sum / iod_count as f64)
}

// ---- Sentence splitting ----

/// Split text into sentences, return word count per sentence.
///
/// Handles: multiple sentences per line, newline-separated sentences,
/// abbreviations (Mr., Dr., e.g., i.e., etc., vs., no., fig., eq.),
/// ellipsis (...), decimal numbers (3.14), and consecutive punctuation (!?).
///
/// Filters out fragments with fewer than 3 words.
fn sentence_word_counts(text: &str) -> Vec<f64> {
    let mut counts = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    let len = bytes.len();

    let mut i = 0;
    while i < len {
        let b = bytes[i];
        if b == b'.' || b == b'!' || b == b'?' {
            // Skip consecutive punctuation (e.g., "!!!", "!?", "...")
            let mut end = i;
            while end + 1 < len && (bytes[end + 1] == b'.' || bytes[end + 1] == b'!' || bytes[end + 1] == b'?') {
                end += 1;
            }

            // Must be followed by whitespace, newline, EOF, or closing quote/paren
            let followed_by_break = end + 1 >= len
                || bytes[end + 1].is_ascii_whitespace()
                || bytes[end + 1] == b'"'
                || bytes[end + 1] == b'\''
                || bytes[end + 1] == b')';

            if b == b'.' && !followed_by_break {
                // Decimal number or mid-word dot — not a sentence boundary
                i = end + 1;
                continue;
            }

            if b == b'.' && followed_by_break {
                // Check for common abbreviations: look at the word before the dot
                let word_before = word_ending_at(text, i);
                if is_abbreviation(word_before) {
                    i = end + 1;
                    continue;
                }

                // Check for single uppercase letter (initials like "J.")
                if word_before.len() == 1 && word_before.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                    i = end + 1;
                    continue;
                }
            }

            if followed_by_break {
                // Next non-whitespace char should be uppercase or EOF for a real sentence break
                // But we'll be lenient — just require the break character
                let sentence = &text[start..=end];
                let wc = sentence.split_whitespace().count();
                if wc >= 3 {
                    counts.push(wc as f64);
                }
                start = end + 1;
            }

            i = end + 1;
        } else {
            i += 1;
        }
    }

    // Trailing text without terminal punctuation.
    if start < len {
        let sentence = &text[start..];
        let wc = sentence.split_whitespace().count();
        if wc >= 3 {
            counts.push(wc as f64);
        }
    }

    counts
}

/// Extract the word immediately before byte position `dot_pos` (the dot).
fn word_ending_at(text: &str, dot_pos: usize) -> &str {
    let before = &text[..dot_pos];
    let word_start = before
        .rfind(|c: char| !c.is_alphanumeric())
        .map(|p| p + 1)
        .unwrap_or(0);
    &before[word_start..dot_pos]
}

/// Common abbreviations that should not end a sentence.
fn is_abbreviation(word: &str) -> bool {
    matches!(
        word.to_ascii_lowercase().as_str(),
        "mr" | "mrs" | "ms" | "dr" | "prof" | "sr" | "jr"
        | "vs" | "etc" | "approx" | "dept" | "est" | "vol"
        | "no" | "fig" | "eq" | "ref" | "sec" | "ch"
        | "st" | "ave" | "blvd"
        // Multi-dot abbreviations handled by consecutive-dot logic
    )
}

// ---- Tokenizer ----

/// Split text into lowercase word tokens. Single-pass, no allocations beyond the vec.
fn tokenize(text: &str) -> Vec<&str> {
    text.split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|s| s.len() > 1)
        .collect()
}

// ---- Stop words ----

fn is_stop_word(w: &str) -> bool {
    matches!(
        w,
        "the" | "a" | "an" | "and" | "or" | "but" | "in" | "on" | "at" | "to" | "for"
        | "of" | "with" | "by" | "from" | "is" | "it" | "its" | "as" | "are" | "was"
        | "were" | "be" | "been" | "being" | "have" | "has" | "had" | "do" | "does"
        | "did" | "will" | "would" | "could" | "should" | "may" | "might" | "shall"
        | "can" | "this" | "that" | "these" | "those" | "i" | "you" | "he" | "she"
        | "we" | "they" | "me" | "him" | "her" | "us" | "them" | "my" | "your"
        | "his" | "our" | "their" | "not" | "no" | "if" | "so" | "than"
    )
}

// ---- Stats helpers ----

fn mean_f64(vals: &[f64]) -> f64 {
    vals.iter().sum::<f64>() / vals.len() as f64
}

fn mean_f64_raw(vals: &[f64]) -> f64 {
    vals.iter().sum::<f64>() / vals.len() as f64
}

fn variance(vals: &[f64], mean: f64) -> f64 {
    vals.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / vals.len() as f64
}

fn variance_raw(vals: &[f64], mean: f64) -> f64 {
    vals.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / vals.len() as f64
}

fn central_moment(vals: &[f64], mean: f64, power: i32) -> f64 {
    vals.iter()
        .map(|&x| (x - mean).powi(power))
        .sum::<f64>()
        / vals.len() as f64
}
