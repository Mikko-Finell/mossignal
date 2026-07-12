#!/usr/bin/env python3
"""Validate the closed, synchronized bead record before its accepted commit."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


def load_issue(bead_id: str) -> dict[str, object]:
    completed = subprocess.run(
        ["br", "show", bead_id, "--json"],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise ValueError(f"br show failed for {bead_id}: {detail}")

    payload = json.loads(completed.stdout)
    if not isinstance(payload, list) or len(payload) != 1:
        raise ValueError(f"br show returned an unexpected record for {bead_id}")
    issue = payload[0]
    if not isinstance(issue, dict):
        raise ValueError(f"br show returned a non-object record for {bead_id}")
    return issue


def load_jsonl_issue(bead_id: str) -> dict[str, object]:
    path = Path(".beads/issues.jsonl")
    if not path.is_file():
        raise ValueError(".beads/issues.jsonl does not exist; run br sync --flush-only")

    matches: list[dict[str, object]] = []
    for number, line in enumerate(path.read_text().splitlines(), start=1):
        if not line.strip():
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError as error:
            raise ValueError(f"invalid JSONL at {path}:{number}: {error}") from error
        if isinstance(record, dict) and record.get("id") == bead_id:
            matches.append(record)

    if len(matches) != 1:
        raise ValueError(
            f"expected one synchronized JSONL record for {bead_id}, found {len(matches)}"
        )
    return matches[0]


def validate(issue: dict[str, object], synchronized: dict[str, object]) -> list[str]:
    errors: list[str] = []
    bead_id = issue.get("id", "requested bead")

    if issue.get("status") != "closed":
        errors.append(f"{bead_id} is not closed")
    if not issue.get("closed_at"):
        errors.append(f"{bead_id} has no closure timestamp")
    if not issue.get("close_reason"):
        errors.append(f"{bead_id} has no concrete close reason")

    notes = issue.get("notes")
    if not isinstance(notes, str) or "make check-final" not in notes:
        errors.append(f"{bead_id} notes do not record make check-final")

    labels = issue.get("labels")
    if isinstance(labels, list):
        workflow_labels = [label for label in labels if str(label).startswith("workflow:")]
        if workflow_labels:
            errors.append(
                f"{bead_id} retains active workflow labels: {', '.join(map(str, workflow_labels))}"
            )

    for field in ("status", "closed_at", "close_reason", "notes", "labels"):
        if synchronized.get(field) != issue.get(field):
            errors.append(f"synchronized JSONL has stale {field} for {bead_id}")

    return errors


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: check_acceptance_record.py <bead-id>", file=sys.stderr)
        return 2

    bead_id = sys.argv[1]
    try:
        errors = validate(load_issue(bead_id), load_jsonl_issue(bead_id))
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(error, file=sys.stderr)
        return 1

    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
