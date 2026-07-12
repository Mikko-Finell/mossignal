#!/usr/bin/env python3

from __future__ import annotations

import re
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
SRC_ROOT = REPO_ROOT / "crates"

FORBIDDEN_TEST_PATTERNS = [
    (re.compile(r"\bdbg!\s*\("), "dbg! macro left in code"),
]

CFG_TEST_RE = re.compile(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]")


def line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def match_brace_block(text: str, open_brace: int) -> int | None:
    depth = 0
    i = open_brace
    in_string = False
    escape = False
    while i < len(text):
        ch = text[i]
        if in_string:
            if escape:
                escape = False
            elif ch == "\\":
                escape = True
            elif ch == '"':
                in_string = False
        else:
            if ch == '"':
                in_string = True
            elif ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
                if depth == 0:
                    return i
        i += 1
    return None


def test_scopes(path: Path) -> list[tuple[int, str]]:
    text = path.read_text()
    if path.name == "tests.rs" or "tests" in path.parts:
        return [(1, text)]

    scopes: list[tuple[int, str]] = []
    for match in CFG_TEST_RE.finditer(text):
        open_brace = text.find("{", match.end())
        if open_brace == -1:
            continue
        close_brace = match_brace_block(text, open_brace)
        if close_brace is None:
            continue
        scopes.append((line_number(text, open_brace) + 1, text[open_brace + 1 : close_brace]))
    return scopes


def iter_rs_files(root: Path) -> list[Path]:
    return sorted(root.rglob("*.rs"))


def collect_test_scope_violations() -> list[str]:
    violations: list[str] = []
    for path in iter_rs_files(SRC_ROOT):
        for base_line, scope in test_scopes(path):
            for pattern, message in FORBIDDEN_TEST_PATTERNS:
                for match in pattern.finditer(scope):
                    violations.append(
                        f"{path.relative_to(REPO_ROOT)}:{base_line + line_number(scope, match.start()) - 1}: {message}"
                    )
    return list(set(violations))


def collect_unwrap_violations() -> list[str]:
    violations: list[str] = []
    for path in iter_rs_files(SRC_ROOT):
        if "test" in path.name or "tests" in path.parts:
            continue
        text = path.read_text()
        
        # Strip comments
        clean_text = re.sub(r"//.*|/\*.*?\*/", "", text, flags=re.S)
        
        for match in re.finditer(r"\.(unwrap|expect)\s*\(", clean_text):
            line = line_number(text, match.start())
            violations.append(
                f"{path.relative_to(REPO_ROOT)}:{line}: raw .unwrap() or .expect() outside test code"
            )
    return violations


def main() -> int:
    violations = (
        collect_test_scope_violations()
        + collect_unwrap_violations()
    )
    if not violations:
        print("Static guardrails check passed.")
        return 0

    print("Static guardrails violations:")
    for violation in violations:
        print(violation)
    return 1


if __name__ == "__main__":
    sys.exit(main())
