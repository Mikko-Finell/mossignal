"""Regression tests for Rust production and test guardrails."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import check_static_guardrails as guardrails


class StaticGuardrailTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "crates/example/src"
        self.source.mkdir(parents=True)
        self.enterContext(patch.object(guardrails, "REPO_ROOT", self.root))
        self.enterContext(patch.object(guardrails, "SRC_ROOT", self.root / "crates"))

    def write(self, text: str, name: str = "lib.rs") -> None:
        (self.source / name).write_text(text, encoding="utf-8")

    def test_line_comments_do_not_hide_production_unwraps(self) -> None:
        self.write('// module documentation\nfn bad() { value.unwrap(); }\n')
        self.assertEqual(
            guardrails.collect_unwrap_violations(),
            ["crates/example/src/lib.rs:2: raw .unwrap() or .expect() outside test code"],
        )

    def test_comments_and_literals_do_not_hide_or_invent_violations(self) -> None:
        self.write('''/* outer /* nested */ remaining .unwrap() */
fn bad() {
    let url = "https://example.com";
    let prose = r##"a quote: " and .unwrap() /* text */"##;
    let character = '}';
    value.expect("required");
}
''')
        self.assertEqual(
            guardrails.collect_unwrap_violations(),
            ["crates/example/src/lib.rs:6: raw .unwrap() or .expect() outside test code"],
        )

    def test_test_only_items_allow_unwraps_without_hiding_later_production(self) -> None:
        self.write('''#[cfg(test)]
mod tests {
    // A comment containing } must not end the test module.
    #[test]
    fn okay() { value.unwrap(); }
}
#[cfg(test)]
fn helper() { value.expect("allowed"); }
#[cfg(test)]
use other::{test_helper};
fn bad() { value.unwrap(); }
''')
        self.assertEqual(
            guardrails.collect_unwrap_violations(),
            ["crates/example/src/lib.rs:11: raw .unwrap() or .expect() outside test code"],
        )

    def test_test_scopes_find_real_debug_macros_past_comments_and_strings(self) -> None:
        self.write('''#[cfg(test)]
mod tests {
    // } dbg!(comment)
    const TEXT: &str = "} dbg!(literal)";
    fn bad() { dbg!(value); }
}
''')
        self.assertEqual(
            guardrails.collect_test_scope_violations(),
            ["crates/example/src/lib.rs:5: dbg! macro left in code"],
        )

    def test_filename_containing_test_does_not_exempt_production(self) -> None:
        self.write('fn bad() { value.unwrap(); }\n', name="latest.rs")
        self.assertEqual(len(guardrails.collect_unwrap_violations()), 1)

    def test_test_only_statements_do_not_confuse_comparisons_with_type_parameters(self) -> None:
        self.write('''fn okay() {
    #[cfg(test)]
    let value = Some(1 < 2).unwrap();
}
''')
        self.assertEqual(guardrails.collect_unwrap_violations(), [])

    def test_comments_between_method_tokens_do_not_hide_unwraps(self) -> None:
        self.write('fn bad() { value. /* comment */ unwrap(); }\n')
        self.assertEqual(len(guardrails.collect_unwrap_violations()), 1)


if __name__ == "__main__":
    unittest.main()
