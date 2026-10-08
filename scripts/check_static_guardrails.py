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
RAW_STRING_RE = re.compile(r'(?:br|cr|r)(#*)"')
CHAR_RE = re.compile(r"'(?:[^\\'\n]|\\(?:u\{[0-9a-fA-F_]+\}|[^\n]))'")
BODY_ITEM_RE = re.compile(
    r"\s*(?:pub(?:\([^)]*\))?\s+)?(?:(?:const|async|unsafe)\s+)*"
    r"(?:fn|mod|impl|struct|enum|trait|union)\b|\s*\{"
)
FIELD_RE = re.compile(r"\s*(?:pub(?:\([^)]*\))?\s+)?\w+\s*[:,]")


def line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def blank(text: str) -> str:
    """Mask non-code without changing offsets or line numbers."""
    return "".join("\n" if character == "\n" else " " for character in text)


def rust_code(text: str) -> str:
    """Mask comments and literals, including nested block comments/raw strings."""
    code = list(text)
    i = 0
    while i < len(text):
        start = i
        if text.startswith("//", i):
            newline = text.find("\n", i)
            i = len(text) if newline < 0 else newline
        elif text.startswith("/*", i):
            depth = 1
            i += 2
            while i < len(text) and depth:
                if text.startswith("/*", i):
                    depth += 1
                    i += 2
                elif text.startswith("*/", i):
                    depth -= 1
                    i += 2
                else:
                    i += 1
        elif (raw := RAW_STRING_RE.match(text, i)) is not None and (
            i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")
        ):
            closing = '"' + raw.group(1)
            end = text.find(closing, raw.end())
            i = len(text) if end < 0 else end + len(closing)
        elif text[i] == '"':
            i += 1
            while i < len(text):
                if text[i] == "\\":
                    i += 2
                elif text[i] == '"':
                    i += 1
                    break
                else:
                    i += 1
            i = min(i, len(text))
        elif (character := CHAR_RE.match(text, i)) is not None:
            i = character.end()
        else:
            i += 1
            continue
        code[start:i] = blank(text[start:i])
    return "".join(code)


def match_brace_block(code: str, open_brace: int) -> int | None:
    depth = 0
    for i in range(open_brace, len(code)):
        if code[i] == "{":
            depth += 1
        elif code[i] == "}":
            depth -= 1
            if depth == 0:
                return i
    return None


def test_ranges(code: str) -> list[tuple[int, int]]:
    """Locate explicitly cfg(test) items and blocks in masked Rust source."""
    ranges: list[tuple[int, int]] = []
    for match in CFG_TEST_RE.finditer(code):
        start = match.end()
        # Additional item attributes may follow cfg(test).
        while (attribute := re.match(r"\s*#\s*\[", code[start:])) is not None:
            end = code.find("]", start + attribute.end())
            if end < 0:
                break
            start = end + 1
        body_item = BODY_ITEM_RE.match(code, start) is not None
        field = FIELD_RE.match(code, start) is not None
        depth = 0
        i = start
        while i < len(code):
            character = code[i]
            if character == "{" and depth == 0 and body_item:
                end = match_brace_block(code, i)
                if end is not None:
                    ranges.append((match.start(), end + 1))
                break
            if character in "([{":
                depth += 1
            elif character in ")]}":
                depth = max(0, depth - 1)
            elif depth == 0 and (character == ";" or (character == "," and field)):
                ranges.append((match.start(), i + 1))
                break
            i += 1
    return ranges


def test_scopes(path: Path) -> list[tuple[int, str]]:
    text = rust_code(path.read_text())
    if path.name == "tests.rs" or "tests" in path.parts:
        return [(1, text)]

    scopes: list[tuple[int, str]] = []
    for start, end in test_ranges(text):
        scopes.append((line_number(text, start), text[start:end]))
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
    return sorted(set(violations))


def collect_unwrap_violations() -> list[str]:
    violations: list[str] = []
    for path in iter_rs_files(SRC_ROOT):
        if path.name == "tests.rs" or "tests" in path.parts or "examples" in path.parts:
            continue
        text = path.read_text()
        clean_text = rust_code(text)
        code = list(clean_text)
        for start, end in test_ranges(clean_text):
            code[start:end] = blank(clean_text[start:end])
        clean_text = "".join(code)
        for match in re.finditer(r"\.\s*(unwrap|expect)\s*\(", clean_text):
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
