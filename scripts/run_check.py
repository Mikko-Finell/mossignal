#!/usr/bin/env python3
"""Run one check quietly, printing captured diagnostics only on failure."""

from __future__ import annotations

import subprocess
import sys


def main() -> int:
    if len(sys.argv) < 3:
        print("ERROR: usage: run_check.py <check-name> <command> [args...]", file=sys.stderr)
        return 2

    name = sys.argv[1]
    command = sys.argv[2:]

    try:
        completed = subprocess.run(
            command,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
    except OSError as error:
        print(f"ERROR: {name} could not start", file=sys.stderr)
        print(f"command: {subprocess.list2cmdline(command)}", file=sys.stderr)
        print(error, file=sys.stderr)
        return 1

    if completed.returncode == 0:
        return 0

    print(f"ERROR: {name} failed", file=sys.stderr)
    print(f"command: {subprocess.list2cmdline(command)}", file=sys.stderr)
    if completed.stdout:
        print(completed.stdout.rstrip(), file=sys.stderr)
    return completed.returncode


if __name__ == "__main__":
    sys.exit(main())
