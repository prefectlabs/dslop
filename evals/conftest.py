from __future__ import annotations

import os
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

import pytest
from pydantic_ai import Agent

MODELS = [
    "openai:gpt-4o",
    "openai:gpt-5.4",
    "anthropic:claude-sonnet-4-5",
    "google-gla:gemini-2.5-flash",
]

DSLOP_BIN = Path(__file__).resolve().parent.parent / "target" / "release" / "dslop"


@dataclass
class DslopResult:
    """Parsed output from a dslop run."""

    raw: str
    exit_code: int
    violations: list[str]
    fixes: dict[str, str]


def run_dslop(text: str) -> DslopResult:
    """Write text to a temp .md file, run dslop, parse output."""
    with tempfile.NamedTemporaryFile(suffix=".md", mode="w", delete=False) as f:
        f.write(text)
        f.flush()
        path = f.name

    result = subprocess.run(
        [str(DSLOP_BIN), path],
        capture_output=True,
        text=True,
        env={**os.environ, "NO_COLOR": "1"},  # inherit env, strip ANSI
    )

    raw = result.stdout + result.stderr
    violations = []
    fixes: dict[str, str] = {}
    in_fix_block = False

    for line in raw.splitlines():
        line = line.strip()
        if line.startswith("fix:"):
            in_fix_block = True
            continue
        if in_fix_block and ": " in line:
            name, guidance = line.split(": ", 1)
            fixes[name.strip()] = guidance.strip()
        elif not in_fix_block and line and not line.startswith("dslop:"):
            violations.append(line)

    Path(path).unlink(missing_ok=True)

    return DslopResult(
        raw=raw,
        exit_code=result.returncode,
        violations=violations,
        fixes=fixes,
    )


@dataclass
class EvalCase:
    """A sloppy text + the dslop error output an LLM must fix."""

    name: str
    text: str
    dslop_output: DslopResult
    violation_families: list[str]


# -- Fixtures --


@pytest.fixture(params=MODELS, ids=lambda m: m.split(":")[-1])
def model(request: pytest.FixtureRequest) -> str:
    return request.param


@pytest.fixture
def rewrite_agent() -> Agent[None, str]:
    return Agent(
        output_type=str,
        system_prompt=(
            "You are a writer. You will be given a piece of text and an error report from a writing quality tool. "
            "Rewrite the text to fix the violations. "
            "Return ONLY the rewritten text, nothing else — no preamble, no explanation."
        ),
    )


# -- Eval cases --

SLOPPY_TEXTS: dict[str, str] = {
    "em-dash": (
        "The architecture is elegant — a masterclass in design. "
        "Performance matters — and we deliver. "
        "This isn't just code — it's craftsmanship."
    ),
    "contrastive": (
        "It's not a tool, it's a platform. "
        "This isn't just fast, it's blazing. "
        "It's not about code, it's about impact."
    ),
    "contrastive-comma": (
        "Design your tools as verbs, not nouns. "
        "They are complements, not substitutes. "
        "We built a schema, not a product. "
        "Ship value, not features."
    ),
    "uniform-rhythm": (
        "The architecture handles scale. The design handles complexity. "
        "The system handles load. Every component works in harmony. "
        "Every module serves its purpose. Every line has meaning. "
        "Users love the interface. Developers love the API. "
        "Teams love the workflow. The solution delivers results. "
        "The platform drives growth. The framework enables innovation."
    ),
    "mixed-slop": (
        "It's not a framework — it's a revolution. "
        "The system is elegant. The code is clean. The tests are thorough. "
        "The docs are complete. The API is intuitive. The deploy is seamless. "
        "Every feature delivers value. Every release improves quality. "
        "Every commit moves us forward. This isn't just software, it's a movement."
    ),
}


def _build_case(name: str, text: str) -> EvalCase:
    result = run_dslop(text)
    families = list(result.fixes.keys())
    return EvalCase(
        name=name,
        text=text,
        dslop_output=result,
        violation_families=families,
    )


@pytest.fixture(
    params=list(SLOPPY_TEXTS.keys()),
    ids=list(SLOPPY_TEXTS.keys()),
)
def eval_case(request: pytest.FixtureRequest) -> EvalCase:
    name = request.param
    return _build_case(name, SLOPPY_TEXTS[name])
