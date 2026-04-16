"""Eval: can an LLM fix sf violations given the error output?

Matrix: MODELS × SLOPPY_TEXTS
Pass criteria: rewritten text produces zero sf violations for the families
that were originally flagged.
"""

from __future__ import annotations

from pydantic_ai import Agent

from conftest import EvalCase, run_sf


async def test_rewrite_fixes_violations(
    model: str,
    eval_case: EvalCase,
    rewrite_agent: Agent[None, str],
):
    prompt = f"""Here is the original text:

{eval_case.text}

Here is the error report from sf:

{eval_case.sf_output.raw}

Rewrite the text to fix all violations."""

    result = await rewrite_agent.run(prompt, model=model)
    rewritten = result.output

    # Run sf on the rewritten text
    recheck = run_sf(rewritten)

    # Check that the originally-flagged violation families are gone
    remaining = [
        family
        for family in eval_case.violation_families
        if family in recheck.fixes
    ]

    assert not remaining, (
        f"Model {model} failed to fix {remaining} for case '{eval_case.name}'.\n"
        f"Original violations: {eval_case.violation_families}\n"
        f"Rewritten text:\n{rewritten}\n"
        f"Recheck output:\n{recheck.raw}"
    )
