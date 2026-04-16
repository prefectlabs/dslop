"""Use GEPA optimize_anything to evolve sf metric fix guidance strings.

Only optimizes the three metric families (sentence-length-cv,
sentence-length-kurtosis, word-freq-dispersion) since pattern violations
(em-dash, contrastive) already pass reliably.

Only uses eval cases that actually trigger metric violations, so GEPA's
budget isn't wasted on cases that already pass.

Run:
    uv run python evals/optimize_guidance.py
"""

from __future__ import annotations

import asyncio
import json
import sys
from pathlib import Path

# Allow importing from evals/ (where conftest.py lives)
sys.path.insert(0, str(Path(__file__).resolve().parent))

import gepa.optimize_anything as oa
from gepa.optimize_anything import GEPAConfig, EngineConfig, optimize_anything
from pydantic_ai import Agent

from conftest import SLOPPY_TEXTS, run_sf, SfResult

EVAL_MODELS = [
    "openai:gpt-4o",
    "openai:gpt-5.4",
    "anthropic:claude-sonnet-4-5",
]

# Only the metric families — these are what we're optimizing.
METRIC_FAMILIES = {"sentence-length-cv", "sentence-length-kurtosis", "word-freq-dispersion"}

SEED_GUIDANCE = {
    "sentence-length-cv": (
        "writing has a metronomic rhythm \u2014 every sentence hits the same beat. "
        "vary structure: combine related ideas, break apart compound ones, "
        "use subordinate clauses or fragments where natural"
    ),
    "sentence-length-kurtosis": (
        "every sentence is roughly the same length \u2014 the writing is structurally "
        "formulaic. let content dictate form: short ideas get short sentences, "
        "complex ideas get complex ones"
    ),
    "word-freq-dispersion": (
        "key words are sprinkled too uniformly \u2014 concentrate terminology where "
        "it's relevant instead of distributing it evenly across the text"
    ),
}

# Fixed guidance for pattern violations (not being optimized).
PATTERN_GUIDANCE = {
    "em-dash": 'rewrite without em-dash (\u2014); use a comma, semicolon, or split into two sentences',
    "contrastive": 'rephrase to avoid "it\'s not X, it\'s Y" construction; state what it *is* directly',
}

REWRITE_SYSTEM_PROMPT = (
    "You are a writer. You will be given a piece of text and an error report "
    "from a writing quality tool. Rewrite the text to fix the violations. "
    "Return ONLY the rewritten text, nothing else \u2014 no preamble, no explanation."
)


# Pre-compute which eval cases actually have metric violations,
# so we don't waste GEPA budget on pure pattern-only cases.
def _build_metric_cases() -> dict[str, tuple[str, SfResult, list[str]]]:
    cases = {}
    for name, text in SLOPPY_TEXTS.items():
        result = run_sf(text)
        metric_violations = [f for f in result.fixes if f in METRIC_FAMILIES]
        if metric_violations:
            cases[name] = (text, result, metric_violations)
    return cases


def build_sf_output(original_result: SfResult, candidate: dict) -> str:
    """Reconstruct sf output with candidate metric guidance + fixed pattern guidance."""
    lines = []

    for line in original_result.raw.splitlines():
        stripped = line.strip()
        if stripped.startswith("fix:"):
            break
        lines.append(line)

    all_guidance = {**PATTERN_GUIDANCE, **candidate}

    lines.append("")
    lines.append("fix:")
    for family in original_result.fixes:
        guidance = all_guidance.get(family, original_result.fixes[family])
        lines.append(f"  {family}: {guidance}")

    return "\n".join(lines)


def evaluate(candidate: dict) -> tuple[float, dict]:
    """Score candidate metric guidance strings. Only tests cases with metric violations."""
    agent = Agent(output_type=str, system_prompt=REWRITE_SYSTEM_PROMPT)
    cases = _build_metric_cases()

    total = 0
    passed = 0
    traces: list[str] = []

    for text_name, (text, original, metric_families) in cases.items():
        patched_output = build_sf_output(original, candidate)

        for model in EVAL_MODELS:
            total += 1
            model_short = model.split(":")[-1]

            prompt = (
                f"Here is the original text:\n\n{text}\n\n"
                f"Here is the error report from sf:\n\n{patched_output}\n\n"
                f"Rewrite the text to fix all violations."
            )

            try:
                result = asyncio.run(agent.run(prompt, model=model))
                rewritten = result.output
                recheck = run_sf(rewritten)

                # Only check if metric violations are fixed (ignore pattern violations)
                remaining = [f for f in metric_families if f in recheck.fixes]

                if not remaining:
                    passed += 1
                    traces.append(f"[PASS] {model_short}/{text_name}")
                else:
                    # Include scores so GEPA's reflection LLM sees how close we got
                    score_details = []
                    for v in recheck.violations:
                        for fam in remaining:
                            if fam in v:
                                score_details.append(v.strip())
                    traces.append(
                        f"[FAIL] {model_short}/{text_name} "
                        f"remaining={remaining}\n"
                        f"  scores: {score_details}\n"
                        f"  rewritten: {rewritten[:300]}"
                    )
            except Exception as e:
                traces.append(f"[ERROR] {model_short}/{text_name}: {e}")

    score = passed / total if total > 0 else 0.0

    oa.log(f"Pass rate: {passed}/{total} = {score:.2%}")
    for trace in traces:
        oa.log(trace)

    return score, {
        "pass_rate": f"{passed}/{total}",
        "failures": [t for t in traces if t.startswith("[FAIL]")],
    }


def main():
    result = optimize_anything(
        seed_candidate=SEED_GUIDANCE,
        evaluator=evaluate,
        objective=(
            "Optimize three fix guidance strings that a writing quality tool shows "
            "to LLMs when their prose fails statistical checks. The three metrics:\n"
            "- sentence-length-cv: coefficient of variation of sentence lengths. "
            "  Fails when CV < 0.3 (too uniform). Human prose is 0.4-0.7.\n"
            "- sentence-length-kurtosis: excess kurtosis of sentence lengths. "
            "  Fails when kurtosis is too low (no outlier sentences).\n"
            "- word-freq-dispersion: index of dispersion of top word frequencies "
            "  across text chunks. Fails when too low (words repeat too evenly).\n\n"
            "The guidance must make LLMs actually restructure their writing so it "
            "passes these statistical checks on re-evaluation. Under 200 chars each. "
            "Tell the LLM what structural pattern is wrong and give concrete "
            "rewriting strategies that change the statistical profile."
        ),
        background=(
            "Current guidance fails ~50% of the time. The main failure mode is that "
            "LLMs interpret the guidance too literally and make superficial changes "
            "(e.g. inserting one random short sentence) instead of genuinely "
            "restructuring. The guidance needs to cause wholesale paragraph-level "
            "rewrites, not local edits. Strategies that work: merging 2-3 related "
            "sentences into one complex one, breaking a dense sentence into a "
            "fragment + elaboration, using parenthetical asides, varying paragraph "
            "length. The LLM only sees the fix guidance once — it has to work on "
            "the first attempt."
        ),
        config=GEPAConfig(
            engine=EngineConfig(max_metric_calls=100),
        ),
    )

    print("\n" + "=" * 60)
    print("OPTIMIZED METRIC GUIDANCE")
    print("=" * 60)

    best = result.best_candidate
    for family, guidance in best.items():
        print(f"\n{family}:")
        print(f"  {guidance}")

    out_path = Path(__file__).parent / "optimized_guidance.json"
    with open(out_path, "w") as f:
        json.dump(best, f, indent=2)
    print(f"\nSaved to {out_path}")


if __name__ == "__main__":
    main()
