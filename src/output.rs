use std::io::Write;

use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use crate::check::FileResult;

pub fn print_results(results: &[FileResult]) {
    let mut stdout = StandardStream::stdout(ColorChoice::Auto);

    let pattern_count: usize = results
        .iter()
        .flat_map(|r| &r.findings)
        .map(|f| f.matches.len())
        .sum();
    let metric_count: usize = results.iter().map(|r| r.metric_violations.len()).sum();
    let total = pattern_count + metric_count;

    stdout
        .set_color(ColorSpec::new().set_fg(Some(Color::Red)).set_bold(true))
        .ok();
    writeln!(
        &mut stdout,
        "sf: {total} violation{s} in {f} file{fs}",
        s = if total == 1 { "" } else { "s" },
        f = results.len(),
        fs = if results.len() == 1 { "" } else { "s" },
    )
    .ok();
    stdout.reset().ok();

    // Collect unique fix messages to print once at the end.
    let mut fixes: Vec<(&str, &str)> = Vec::new();

    for result in results {
        for finding in &result.findings {
            for m in &finding.matches {
                writeln!(
                    &mut stdout,
                    "  {}:{}:{} {}",
                    result.path, m.line_number, m.column, finding.pattern_name,
                )
                .ok();
            }
            if !fixes.iter().any(|(name, _)| *name == finding.pattern_name) {
                fixes.push((finding.pattern_name, finding.fix));
            }
        }
        for mv in &result.metric_violations {
            writeln!(
                &mut stdout,
                "  {} {}={:.2} (threshold {:.2})",
                result.path, mv.metric_name, mv.score, mv.threshold,
            )
            .ok();
            if !fixes.iter().any(|(name, _)| *name == mv.metric_name) {
                fixes.push((mv.metric_name, mv.fix));
            }
        }
    }

    // Fix guidance block — one line per violation type.
    if !fixes.is_empty() {
        writeln!(&mut stdout).ok();
        stdout
            .set_color(ColorSpec::new().set_bold(true))
            .ok();
        writeln!(&mut stdout, "fix:").ok();
        stdout.reset().ok();
        for (name, fix) in &fixes {
            writeln!(&mut stdout, "  {name}: {fix}").ok();
        }
    }
}
