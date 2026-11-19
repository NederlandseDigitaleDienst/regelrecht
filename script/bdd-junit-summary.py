#!/usr/bin/env python3
"""Summarise the JUnit reports of the BDD runners as Markdown.

The runners write a report when BDD_JUNIT names a file
(packages/engine/tests/bdd/runner.rs). The reusable workflow
.github/workflows/corpus-bdd.yml passes one report per run and appends the
output to the step summary:

    script/bdd-junit-summary.py corpus.xml constructs.xml >> "$GITHUB_STEP_SUMMARY"

A table with the counts per report, then every failed scenario with the first
line of its message. A report that is missing or unreadable is listed as such:
the run that should have written it stopped before its last scenario, and the
summary says so rather than leaving the row out. The exit status is always 0;
whether the job fails is up to the runs themselves.
"""

import sys
import xml.etree.ElementTree as ET
from pathlib import Path

# The first line of a failure message is enough to see which step broke; the
# whole message is in the report.
MESSAGE_WIDTH = 200


def summarise(path: Path) -> tuple[str, list[str]]:
    """The table row for the report at `path`, and a line per failed scenario."""
    try:
        root = ET.parse(path).getroot()
    except (OSError, ET.ParseError) as error:
        return f"| `{path.name}` | no report: {error} | | | |", []

    passed = failed = skipped = 0
    failures = []
    for case in root.iter("testcase"):
        problem = case.find("failure")
        if problem is None:
            problem = case.find("error")
        if problem is not None:
            failed += 1
            text = (problem.get("message") or problem.text or "").strip()
            first = text.splitlines()[0][:MESSAGE_WIDTH] if text else ""
            failures.append(f"- `{path.name}`: {case.get('name', '?')}: {first}")
        elif case.find("skipped") is not None:
            skipped += 1
        else:
            passed += 1
    total = passed + failed + skipped
    return f"| `{path.name}` | {total} | {passed} | {failed} | {skipped} |", failures


def main(paths: list[str]) -> None:
    rows, failures = [], []
    for name in paths:
        row, failed = summarise(Path(name))
        rows.append(row)
        failures.extend(failed)

    print("## BDD scenarios\n")
    print("| Report | Scenarios | Passed | Failed | Skipped |")
    print("| --- | ---: | ---: | ---: | ---: |")
    print("\n".join(rows))
    if failures:
        print("\n### Failed scenarios\n")
        print("\n".join(failures))


if __name__ == "__main__":
    main(sys.argv[1:])
