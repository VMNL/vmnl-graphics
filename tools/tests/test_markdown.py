# SPDX-FileCopyrightText: 2026 VMNL contributors
# SPDX-License-Identifier: MIT

"""Regression coverage for Markdown rules, literal regions, discovery and CLI failures."""

import contextlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location(
    "check_markdown", Path(__file__).resolve().parents[1] / "check_markdown.py")
assert SPEC is not None and SPEC.loader is not None
markdown = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(markdown)


class ConventionTests(unittest.TestCase):
    def check(self, text):
        return markdown.check_content(text.encode("utf-8"))

    def test_valid_document_and_aligned_nested_list(self):
        self.assertEqual(self.check(
            "# Guide\n\nÉcriture Unicode.\n\n- Parent\n  continuation.\n  - Child\n"
            "    child continuation.\n  parent continuation.\n\n1. First.\n"
            "   continuation.\n2. Second.\n\n| Name | Value |\n| :--- | ---: |\n"
            "| `x` | a \\| b |\n"), [])

    def test_reports_rule_and_exact_line(self):
        self.assertIn((3, "use '- ' for unordered lists"), self.check("# Guide\n\n* Item\n"))

    def test_rejects_invalid_bytes_line_endings_and_final_newline(self):
        for data in (b"\xff", b"# Guide\r\n", b"# Guide", b"# Guide\n\n", b""):
            with self.subTest(data=data):
                self.assertTrue(markdown.check_content(data))

    def test_rejects_spacing_headings_list_markers_and_indentation(self):
        for text in ("#Bad\n", "# Guide\nText\n", "Text\n\n\nText\n",
                     "Text\n#Bad\n",
                     "Title\n=====\n", "Text \n", "Text\t\n", "Text   \n",
                     "+ Item\n", "1) Item\n", "- Item\n continuation\n",
                     "Text\n- Item\n", "- Item\nText\n",
                     "Text\n\n- Item\n\n" + "word " * 30 + "end\n"):
            with self.subTest(text=text):
                self.assertTrue(self.check(text))

    def test_rejects_long_prose_but_preserves_indivisible_tokens(self):
        self.assertTrue(self.check("word " * 21 + "end\n"))
        for text in ("https://example.com/" + "a" * 120 + "\n",
                     "- `" + "a b " * 35 + "`\n"):
            with self.subTest(text=text):
                self.assertEqual(self.check(text), [])

    def test_exempts_entire_image_and_hardbreak_paragraphs_from_wrapping(self):
        long = "word " * 30 + "end"
        for marker in ("![caption](image.png)", "hard break  ", "hard break\\"):
            with self.subTest(marker=marker):
                self.assertEqual(self.check(long + "\n" + marker + "\n"), [])

    def test_wrapping_exceptions_do_not_leak_between_list_items(self):
        self.assertTrue(self.check("- ![caption](image.png)\n- " + "word " * 30 + "end\n"))
        self.assertTrue(self.check("- Hard break  \n- " + "word " * 30 + "end\n"))

    def test_horizontal_rules_are_not_list_items(self):
        self.assertEqual(self.check("# Guide\n\n* * *\n\n---\n"), [])

    def test_blockquote_and_github_reference_continuation(self):
        self.assertEqual(self.check(
            "> " + "word " * 30 + "end\n\nSee [CI run\n#12345](https://example.com).\n"), [])
        self.assertEqual(self.check("- See [CI run\n  #12345](https://example.com).\n"), [])

    def test_tables_require_spacing_three_hyphens_and_consistent_cells(self):
        for table in ("| A | B |\n|---|---|\n", "| A | B |\n| ---- | --- |\n",
                      "| A | B |\n| --- | --- |\n| a |\n",
                      "Text\n| A | B |\n| --- | --- |\n",
                      "| A | B |\n| --- | --- |\nText\n"):
            with self.subTest(table=table):
                self.assertTrue(self.check(table))
        self.assertEqual(self.check("| A | B |\n| --- | --- |\n| a | " + "b" * 150 + " |\n"), [])

    def test_escaped_table_pipes_respect_backslash_parity(self):
        self.assertEqual(markdown.cells(r"| a \| b | c |"), [r" a \| b ", " c "])
        self.assertEqual(markdown.cells(r"| a \\| b |"), [" a \\\\", " b "])

    def test_preserves_fenced_code_longer_fences_and_embedded_shorter_fences(self):
        for opener, closer in (("```rust", "```"), ("~~~~", "~~~~~"),
                               ("````markdown", "````")):
            with self.subTest(opener=opener):
                text = "# Guide\n\n" + opener + "\n#NoSpace\n\n\n* literal   \n" + "x" * 130
                if opener.startswith("````"):
                    text += "\n```\nText\n```"
                self.assertEqual(self.check(text + "\n" + closer + "\n"), [])
        self.assertTrue(self.check("Text\n```text\ncode\n```\n"))
        self.assertTrue(self.check("```text\ncode\n```\nText\n"))

    def test_nested_fenced_and_indented_code_remain_literal(self):
        self.assertEqual(self.check(
            "- Example:\n\n  ```text\n  * literal   \n\n\n  #NoSpace\n  ```\n"
            "\n- Other:\n\n      * literal   \n\n      #NoSpace\n"), [])
        self.assertEqual(self.check("    * literal   \n\n\n    #NoSpace\n"), [])
        self.assertEqual(self.check("- ```text\n  * literal   \n  ```\n"), [])

    def test_front_matter_html_and_comments_are_preserved(self):
        for literal in ("---\ndescription: " + "word " * 30 + "\n---\n",
                        "<div>\n* literal   \n",
                        "<custom-tag>\n* literal   \n",
                        "<!--\n#NoSpace\n\n\n-->\n",
                        "<script>\n\n\n#NoSpace\n</script>\n"):
            with self.subTest(literal=literal):
                self.assertEqual(self.check(literal + "\n# Guide\n"), [])

    def test_generated_documents_are_owned_by_generators(self):
        self.assertEqual(self.check("<!-- Generated by tool; do not edit. -->\n#Title\n\n\n"), [])

    def test_indented_prose_after_list_is_checked_as_prose(self):
        self.assertEqual(self.check("  - Item.\n\n  A separate paragraph.\n"), [])
        self.assertTrue(self.check("  - Item.\n\n  " + "word " * 30 + "end\n"))


class DiscoveryAndCliTests(unittest.TestCase):
    def test_includes_hidden_new_files_and_prunes_outputs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ("new guide.md", ".agents/guide.MD", "docs/guide.md",
                         "target/output.md", "examples/target/output.md", ".git/hidden.md",
                         "vendor/README.md", "third_party/README.md", "node_modules/README.md"):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("# Guide\n", encoding="utf-8")
            self.assertEqual([p.relative_to(root).as_posix() for p in markdown.markdown_files(root)],
                             [".agents/guide.MD", "docs/guide.md", "new guide.md"])

    def test_cli_exit_status_diagnostics_and_no_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "guide.md"
            for data, status in ((b"# Guide\n", 0), (b"# Guide\n\n* Item\n", 1)):
                with self.subTest(data=data):
                    path.write_bytes(data)
                    output, error = io.StringIO(), io.StringIO()
                    with contextlib.redirect_stdout(output), contextlib.redirect_stderr(error):
                        self.assertEqual(markdown.main(["--root", directory]), status)
                    self.assertEqual(path.read_bytes(), data)
                    if status:
                        self.assertIn("guide.md:3:", error.getvalue())
                    else:
                        self.assertIn("1 files checked", output.getvalue())

    def test_empty_root_fails_instead_of_passing_without_checks(self):
        with tempfile.TemporaryDirectory() as directory, contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(markdown.main(["--root", directory]), 1)


if __name__ == "__main__":
    unittest.main()
