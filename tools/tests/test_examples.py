# SPDX-FileCopyrightText: 2026 VMNL contributors
# SPDX-License-Identifier: MIT

import contextlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    "vmnl_examples", Path(__file__).resolve().parents[1] / "examples.py"
)
examples = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(examples)


class ExampleToolTests(unittest.TestCase):
    def setUp(self):
        workspace = tempfile.TemporaryDirectory()
        self.addCleanup(workspace.cleanup)
        self.root = Path(workspace.name)
        self.manifest = self.root / "Cargo.toml"
        self.catalogue = self.root / "examples/README.md"
        self.manifest.write_text("[workspace]\nmembers = [\n]\n", encoding="utf-8")
        for domain in examples.DOMAINS:
            (self.root / "examples" / domain).mkdir(parents=True)
        self.catalogue.write_text(
            examples.CATALOGUE_HEADER + "\n| --- | --- | --- |\n", encoding="utf-8"
        )

    def create(self, domain, name, dry_run=False):
        with contextlib.redirect_stdout(io.StringIO()):
            examples.create(self.root, domain, name, dry_run, "Test Author")

    def snapshot(self):
        return {
            path.relative_to(self.root): path.read_bytes()
            for path in self.root.rglob("*")
            if path.is_file()
        }

    def test_registers_both_templates(self):
        self.create("d2", "rectangle")
        self.create("raw", "triangle")
        self.assertEqual(examples.check(self.root), [])
        rectangle = (self.root / "examples/d2/rectangle/src/main.rs").read_text()
        triangle = (self.root / "examples/raw/triangle/src/main.rs").read_text()
        self.assertIn("Shape::rect", rectangle)
        self.assertIn("Test Author", rectangle)
        self.assertIn("draw_raw_2d", triangle)

    def test_dry_run_and_duplicate_leave_workspace_unchanged(self):
        before = self.snapshot()
        self.create("d2", "rectangle", dry_run=True)
        self.assertEqual(self.snapshot(), before)
        self.create("d2", "rectangle")
        after = self.snapshot()
        with self.assertRaisesRegex(ValueError, "already exists"):
            self.create("d2", "rectangle")
        self.assertEqual(self.snapshot(), after)

    def test_detects_stale_source_link(self):
        self.create("d2", "rectangle")
        path = self.root / "examples/d2/rectangle/src/main.rs"
        path.unlink()
        self.assertIn("d2_rectangle: missing or incorrect source link", examples.check(self.root))

    def test_rolls_back_a_failed_source_write(self):
        before = self.snapshot()
        write_text = Path.write_text

        def reject_source(path, *args, **kwargs):
            if path.name == "main.rs":
                raise OSError("fixture write failed")
            return write_text(path, *args, **kwargs)

        with patch.object(Path, "write_text", reject_source):
            with self.assertRaisesRegex(OSError, "fixture write failed"):
                self.create("d2", "rectangle")
        self.assertEqual(self.snapshot(), before)
        self.assertFalse((self.root / "examples/d2/rectangle").exists())

    def test_rolls_back_manifest_when_catalogue_write_fails(self):
        before = self.snapshot()
        replace_text = examples.replace_text

        def reject_catalogue(path, content):
            if path == self.catalogue:
                raise OSError("catalogue write failed")
            replace_text(path, content)

        with patch.object(examples, "replace_text", reject_catalogue):
            with self.assertRaisesRegex(OSError, "catalogue write failed"):
                self.create("d2", "rectangle")
        self.assertEqual(self.snapshot(), before)
        self.assertFalse((self.root / "examples/d2/rectangle").exists())


if __name__ == "__main__":
    unittest.main()
