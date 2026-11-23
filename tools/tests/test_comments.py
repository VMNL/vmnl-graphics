# SPDX-FileCopyrightText: 2026 Hugo Duda
# SPDX-License-Identifier: MIT

from pathlib import Path
import tempfile
import unittest

from tools import check_comments


class SourceCommentTests(unittest.TestCase):
    def errors(self, source: str) -> list[str]:
        return check_comments.source_errors(Path("lib.rs"), source)

    def test_allows_optional_sections_and_ignores_ordinary_comments(self) -> None:
        self.assertEqual(self.errors("""\
//! Describes the library.
//// # Panics
// # Panics
/// Returns the active mode.
pub fn mode() {}
"""), [])

    def test_accepts_rust_fence_and_ignores_headings_inside_it(self) -> None:
        self.assertEqual(self.errors("""\
/// # Arguments
/// - `context` selects the device.
/// # Errors
/// Returns an error if device creation fails.
/// # Safety
/// Keep the device alive.
/// # Example
/// ````rust,no_run
/// # fn main() {
/// let code = r"
/// ```
/// # Panics
/// # Arguments
/// ";
/// # }
/// ````
"""), [])

    def test_reports_section_violations_with_source_lines(self) -> None:
        self.assertEqual(self.errors("""\
/// # Safety
/// Keep the device alive.
/// # Errors ##
///
/// # Safety
/// Keep the device alive.
/// # Panics
/// Description.
"""), [
            "lib.rs:3: # Errors section is out of order",
            "lib.rs:3: empty # Errors section",
            "lib.rs:5: duplicate # Safety section",
            "lib.rs:7: # Panics is forbidden in library Rustdoc",
        ])

    def test_duplicate_sections_in_different_blocks_are_independent(self) -> None:
        self.assertEqual(self.errors("""\
//! # Example
//! See [the workflow](Self#example).
/// # Example
/// See [the workflow](Self#example).
fn sample() {}

/// # Example
/// See [the workflow](Self#example).
fn another() {}
"""), [])

    def test_accepts_direct_example_links(self) -> None:
        for link in ("Self#example", "Shape#example-circle-sector", "struct@crate::Window#example"):
            with self.subTest(link=link):
                self.assertEqual(self.errors(f"""\
/// # Example
///
/// See [the shared
/// workflow]({link}).
"""), [])

    def test_rejects_prose_without_an_example(self) -> None:
        for content in (
            "See the workflow on [Self].",
            "See the [method](Self#method.mode).",
            "See the [guide](https://example.test#example).",
            r"Show \[the syntax](Self#example).",
            "Show `[the syntax](Self#example)`.",
            "![Illustration](Self#example).",
            "```text\n/// [Illustration](Self#example)\n/// ```",
        ):
            with self.subTest(content=content):
                self.assertEqual(self.errors(f"/// # Example\n/// {content}\n"), [
                    "lib.rs:1: # Example needs Rust code or a direct example link",
                ])

    def test_rejects_empty_and_unclosed_examples(self) -> None:
        for content in ("```rust\n/// ```", "```rust\n/// let result = mode();"):
            with self.subTest(content=content):
                self.assertEqual(self.errors(f"/// # Example\n/// {content}\n"), [
                    "lib.rs:1: empty # Example section",
                ])

    def test_source_scan_reports_all_files_in_order(self) -> None:
        with tempfile.TemporaryDirectory() as value:
            root = Path(value)
            source = root / "crates/example/src"
            source.mkdir(parents=True)
            (source / "z.rs").write_text("/// # Panics\n", encoding="utf-8")
            (source / "a.rs").write_text("/// # Arguments\n", encoding="utf-8")
            (root / "outside.rs").write_text("/// # Panics\n", encoding="utf-8")

            self.assertEqual(check_comments.check_sources(root), [
                "crates/example/src/a.rs:1: empty # Arguments section",
                "crates/example/src/z.rs:1: # Panics is forbidden in library Rustdoc",
            ])


class RenderedExampleTests(unittest.TestCase):
    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.crate = Path(directory.name) / "vmnl_graphics"
        self.crate.mkdir()

    def write_page(self, name: str, content: str) -> None:
        (self.crate / name).write_text(content, encoding="utf-8")

    def test_accepts_example_links_to_same_and_other_pages(self) -> None:
        self.write_page("shape.html", """\
<h2 id="example">Example</h2>
<a href="#example">Local</a>
<a href="rect.html#example-2">Shared</a>
<a href="https://example.test/rect.html#example-missing">External</a>
<a href="rect.html#method.unchecked">Other section</a>
""")
        self.write_page("rect.html", "<section id='example-2'>Example</section>")

        self.assertEqual(check_comments.check_rendered_examples(self.crate), [])

    def test_rejects_missing_pages_and_anchors(self) -> None:
        self.write_page("shape.html", """\
<a href="#example-missing">Local</a>
<a href="rect.html#example">Shared</a>
<a href="absent.html#example">Shared</a>
""")
        self.write_page("rect.html", '<section data-id="example">Missing anchor</section>')

        self.assertEqual(check_comments.check_rendered_examples(self.crate), [
            "vmnl_graphics/shape.html:1: missing Rustdoc anchor for example link #example-missing",
            "vmnl_graphics/shape.html:2: missing Rustdoc anchor for example link rect.html#example",
            "vmnl_graphics/shape.html:3: missing Rustdoc page for example link absent.html#example",
        ])

    def test_rejects_missing_rustdoc(self) -> None:
        self.assertEqual(check_comments.check_rendered_examples(self.crate), [
            f"{self.crate.resolve()}: missing Rustdoc pages",
        ])
