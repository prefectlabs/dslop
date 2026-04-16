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

SF_BIN = Path(__file__).resolve().parent.parent / "target" / "release" / "sf"


@dataclass
class SfResult:
    """Parsed output from a sf run."""

    raw: str
    exit_code: int
    violations: list[str]
    fixes: dict[str, str]


def run_sf(text: str) -> SfResult:
    """Write text to a temp .md file, run sf, parse output."""
    with tempfile.NamedTemporaryFile(suffix=".md", mode="w", delete=False) as f:
        f.write(text)
        f.flush()
        path = f.name

    result = subprocess.run(
        [str(SF_BIN), path],
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
        elif not in_fix_block and line and not line.startswith("sf:"):
            violations.append(line)

    Path(path).unlink(missing_ok=True)

    return SfResult(
        raw=raw,
        exit_code=result.returncode,
        violations=violations,
        fixes=fixes,
    )


@dataclass
class EvalCase:
    """A sloppy text + the sf error output an LLM must fix."""

    name: str
    text: str
    sf_output: SfResult
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
    result = run_sf(text)
    families = list(result.fixes.keys())
    return EvalCase(
        name=name,
        text=text,
        sf_output=result,
        violation_families=families,
    )


@pytest.fixture(
    params=list(SLOPPY_TEXTS.keys()),
    ids=list(SLOPPY_TEXTS.keys()),
)
def eval_case(request: pytest.FixtureRequest) -> EvalCase:
    name = request.param
    return _build_case(name, SLOPPY_TEXTS[name])
