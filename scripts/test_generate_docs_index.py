import unittest
import os
import sys
import json
import tempfile

# Add scripts directory to path to allow importing generate_docs_index
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import generate_docs_index

class TestGenerateDocsIndex(unittest.TestCase):
    def test_metadata_uses_defines(self):
        content = (
            "# Document title\n\n"
            "**Status:** Draft  \n"
            "**Defines:** Signals, processors, and runtime behavior  \n"
        )

        self.assertEqual(
            generate_docs_index.parse_metadata(content, "specs/example.md"),
            ("Document title", "Signals, processors, and runtime behavior"),
        )

    def test_metadata_requires_defines(self):
        content = "# Document title\n\n**Summary:** Old metadata\n"

        with self.assertRaisesRegex(ValueError, r"Missing or empty '\*\*Defines:\*\*'"):
            generate_docs_index.parse_metadata(content, "specs/example.md")

    def test_index_json_contains_defines_without_summary_or_tags(self):
        generated = generate_docs_index.generate_index_json([{
            "rel_path": os.path.join("specs", "example.md"),
            "title": "Example",
            "defines": "Example behavior",
            "line_count": 1,
            "headings": [],
        }])

        document = json.loads(generated)["documents"][0]
        self.assertEqual(document["path"], "docs/specs/example.md")
        self.assertEqual(document["defines"], "Example behavior")
        self.assertNotIn("summary", document)
        self.assertNotIn("tags", document)

    def test_get_indexed_docs_includes_archive(self):
        original_docs_dir = generate_docs_index.docs_dir
        with tempfile.TemporaryDirectory() as temp_dir:
            docs_dir = os.path.join(temp_dir, "docs")
            archive_dir = os.path.join(docs_dir, "archive")
            specs_dir = os.path.join(docs_dir, "specs")
            os.makedirs(archive_dir)
            os.makedirs(specs_dir)
            open(os.path.join(archive_dir, "old.md"), "w").close()
            open(os.path.join(specs_dir, "current.md"), "w").close()
            open(os.path.join(docs_dir, "ignored.txt"), "w").close()
            generate_docs_index.docs_dir = docs_dir
            try:
                paths = [rel_path for _, rel_path in generate_docs_index.get_indexed_docs()]
            finally:
                generate_docs_index.docs_dir = original_docs_dir

        self.assertEqual(paths, ["archive/old.md", "specs/current.md"])

    def assert_structural_invariants(self, headings, line_count):
        """
        Verify the structural invariants for headings:
        - 1 <= start_line <= end_line <= line_count
        - Heading start lines are strictly increasing
        """
        prev_start = 0
        for h in headings:
            self.assertTrue(1 <= h["start_line"], f"start_line {h['start_line']} < 1")
            self.assertTrue(h["start_line"] <= h["end_line"], f"start_line {h['start_line']} > end_line {h['end_line']}")
            self.assertTrue(h["end_line"] <= line_count, f"end_line {h['end_line']} > line_count {line_count}")
            self.assertTrue(h["start_line"] > prev_start, f"start_lines not strictly increasing: {h['start_line']} <= {prev_start}")
            prev_start = h["start_line"]

    def test_normal_nested_headings(self):
        content = (
            "# Main Title\n"          # 1
            "Some intro text.\n"      # 2
            "## Section 1\n"          # 3
            "Section 1 text.\n"       # 4
            "### Subsection 1.1\n"    # 5
            "Sub 1.1 text.\n"         # 6
            "## Section 2\n"          # 7
            "Section 2 text.\n"       # 8
        )
        line_count = len(content.splitlines())
        headings = generate_docs_index.parse_headings(content)
        self.assert_structural_invariants(headings, line_count)

        self.assertEqual(len(headings), 4)
        
        # H1: # Main Title (lines 1 to 8)
        self.assertEqual(headings[0]["level"], 1)
        self.assertEqual(headings[0]["title"], "Main Title")
        self.assertEqual(headings[0]["start_line"], 1)
        self.assertEqual(headings[0]["end_line"], 8)

        # H2: ## Section 1 (lines 3 to 6)
        self.assertEqual(headings[1]["level"], 2)
        self.assertEqual(headings[1]["title"], "Section 1")
        self.assertEqual(headings[1]["start_line"], 3)
        self.assertEqual(headings[1]["end_line"], 6)

        # H3: ### Subsection 1.1 (lines 5 to 6)
        self.assertEqual(headings[2]["level"], 3)
        self.assertEqual(headings[2]["title"], "Subsection 1.1")
        self.assertEqual(headings[2]["start_line"], 5)
        self.assertEqual(headings[2]["end_line"], 6)

        # H2: ## Section 2 (lines 7 to 8)
        self.assertEqual(headings[3]["level"], 2)
        self.assertEqual(headings[3]["title"], "Section 2")
        self.assertEqual(headings[3]["start_line"], 7)
        self.assertEqual(headings[3]["end_line"], 8)

    def test_no_headings(self):
        content = "Some document without any headings.\nJust plain text."
        line_count = len(content.splitlines())
        headings = generate_docs_index.parse_headings(content)
        self.assert_structural_invariants(headings, line_count)
        self.assertEqual(headings, [])

    def test_fenced_code_blocks_ignored(self):
        content = (
            "# Document Title\n"       # 1
            "```\n"                    # 2
            "# Ignored Heading\n"      # 3
            "```\n"                    # 4
            "## Real Heading\n"        # 5
            "~~~\n"                    # 6
            "## Ignored Tilde\n"       # 7
            "~~~\n"                    # 8
        )
        line_count = len(content.splitlines())
        headings = generate_docs_index.parse_headings(content)
        self.assert_structural_invariants(headings, line_count)

        self.assertEqual(len(headings), 2)
        
        self.assertEqual(headings[0]["title"], "Document Title")
        self.assertEqual(headings[0]["level"], 1)
        self.assertEqual(headings[0]["start_line"], 1)
        self.assertEqual(headings[0]["end_line"], 8)

        self.assertEqual(headings[1]["title"], "Real Heading")
        self.assertEqual(headings[1]["level"], 2)
        self.assertEqual(headings[1]["start_line"], 5)
        self.assertEqual(headings[1]["end_line"], 8)

    def test_fences_various_lengths(self):
        content = (
            "# Title\n"                # 1
            "````\n"                   # 2
            "```\n"                    # 3
            "# Inside 4-backtick block\n" # 4
            "````\n"                   # 5
            "## Next Section\n"        # 6
        )
        line_count = len(content.splitlines())
        headings = generate_docs_index.parse_headings(content)
        self.assert_structural_invariants(headings, line_count)

        self.assertEqual(len(headings), 2)
        self.assertEqual(headings[0]["title"], "Title")
        self.assertEqual(headings[0]["start_line"], 1)
        self.assertEqual(headings[0]["end_line"], 6)

        self.assertEqual(headings[1]["title"], "Next Section")
        self.assertEqual(headings[1]["start_line"], 6)
        self.assertEqual(headings[1]["end_line"], 6)

    def test_closing_fence_exact_rules(self):
        # A closing fence must be at least as long as the opening fence and of the same char,
        # and contain only permitted whitespace.
        content = (
            "# Title\n"                # 1
            "````\n"                   # 2
            "```` extra text\n"        # 3 (Not a valid closing fence, has extra text)
            "```\n"                    # 4 (Not a valid closing fence, too short)
            "````\n"                   # 5 (Valid closing fence)
            "## Real Heading\n"        # 6
        )
        line_count = len(content.splitlines())
        headings = generate_docs_index.parse_headings(content)
        self.assert_structural_invariants(headings, line_count)

        self.assertEqual(len(headings), 2)
        self.assertEqual(headings[0]["title"], "Title")
        self.assertEqual(headings[0]["start_line"], 1)
        self.assertEqual(headings[1]["title"], "Real Heading")
        self.assertEqual(headings[1]["start_line"], 6)

    def test_trailing_hashes_normalization(self):
        content = (
            "# Title with Trailing Hashes ##\n"   # 1
            "## Subtitle #\n"                      # 2
            "### Header ###  \n"                   # 3
            "#### Heading with # inside #\n"        # 4
        )
        line_count = len(content.splitlines())
        headings = generate_docs_index.parse_headings(content)
        self.assert_structural_invariants(headings, line_count)

        self.assertEqual(len(headings), 4)
        self.assertEqual(headings[0]["title"], "Title with Trailing Hashes")
        self.assertEqual(headings[1]["title"], "Subtitle")
        self.assertEqual(headings[2]["title"], "Header")
        self.assertEqual(headings[3]["title"], "Heading with # inside")

    def test_whitespace_tolerance_in_atx_headings(self):
        content = (
            "  # Indented Title\n"                 # 1
            "##NoSpaceAfter\n"                     # 2 (Not a heading under ATX spec)
            "\t### Tab Space\n"                    # 3 (Not a heading, leading tab is 4 chars/different)
            "   ## Indented Heading\n"             # 4 (Valid: 3 leading spaces)
            "    # Too Many Spaces\n"              # 5 (Not a heading: 4 leading spaces)
        )
        line_count = len(content.splitlines())
        headings = generate_docs_index.parse_headings(content)
        self.assert_structural_invariants(headings, line_count)

        self.assertEqual(len(headings), 2)
        self.assertEqual(headings[0]["title"], "Indented Title")
        self.assertEqual(headings[0]["start_line"], 1)
        
        self.assertEqual(headings[1]["title"], "Indented Heading")
        self.assertEqual(headings[1]["start_line"], 4)

if __name__ == "__main__":
    unittest.main()
