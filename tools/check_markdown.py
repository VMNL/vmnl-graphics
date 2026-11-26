#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 VMNL contributors
# SPDX-License-Identifier: MIT

"""Check the mechanical rules in docs/markdown_style.md without modifying files."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
EXCLUDED = {".git", "target", "vendor", "third_party", "third-party", "node_modules",
            ".venv", "__pycache__"}
FENCE = re.compile(r"^(\s*)(`{3,}|~{3,})(.*)$")
ITEM = re.compile(r"^( *)([-+*]|\d+[.)])( +)(.*)$")
HEADING = re.compile(r"^ {0,3}#{1,6}(?:\s|$)")
HTML = re.compile(
    r"^ {0,3}</?(?:address|article|aside|base|blockquote|body|caption|center|col|colgroup|"
    r"dd|details|dialog|dir|div|dl|dt|fieldset|figcaption|figure|footer|form|frame|frameset|"
    r"h[1-6]|head|header|hr|html|iframe|legend|li|link|main|menu|nav|ol|p|param|section|"
    r"source|summary|table|tbody|td|tfoot|th|thead|title|tr|track|ul)(?:\s|/?>)", re.I)


def markdown_files(root: Path) -> list[Path]:
    """Include new files and hidden agent guides; prune outputs and vendored trees."""
    files = []
    for directory, dirs, names in os.walk(root, followlinks=False):
        dirs[:] = sorted(d for d in dirs if d not in EXCLUDED
                         and not (Path(directory) / d).is_symlink())
        files.extend(Path(directory) / name for name in sorted(names)
                     if Path(name).suffix.lower() == ".md"
                     and not (Path(directory) / name).is_symlink())
    return sorted(files)


def cells(line: str) -> list[str]:
    # A pipe preceded by an odd number of backslashes is literal cell content.
    parts, start = [], 0
    for index, character in enumerate(line):
        if character != "|":
            continue
        slashes, previous = 0, index - 1
        while previous >= 0 and line[previous] == "\\":
            slashes += 1
            previous -= 1
        if slashes % 2 == 0:
            parts.append(line[start:index])
            start = index + 1
    parts.append(line[start:])
    if line.startswith("|"):
        parts.pop(0)
    if line.endswith("|") and parts[-1] == "":
        parts.pop()
    return parts


def separator(line: str) -> bool:
    row = cells(line.strip())
    return "|" in line and bool(row) and all(re.fullmatch(r"\s*:?-+:?\s*", c) for c in row)


def literal_lines(lines: list[str]) -> set[int]:
    """Recognize literal regions before applying prose rules, including nested fences."""
    protected = set()
    indented = []
    fence = None
    html_end = None
    list_indent = None
    index = 0
    if lines and lines[0] == "---":
        end = next((i for i in range(1, len(lines)) if lines[i] in ("---", "...")), None)
        if end is not None:
            protected.update(range(end + 1))
            index = end + 1
    for index in range(index, len(lines)):
        line = lines[index]
        if html_end is not None:
            if html_end == "blank" and not line.strip():
                html_end = None
            else:
                protected.add(index)
                if html_end != "blank" and re.search(html_end, line, re.I):
                    html_end = None
                continue
        if fence:
            protected.add(index)
            if re.fullmatch(r"\s*" + re.escape(fence[0]) + "{" + str(len(fence)) + r",}\s*", line):
                fence = None
            continue
        indent = len(line) - len(line.lstrip(" "))
        if line.strip() and (line.startswith("\t") or indent >= (list_indent or 0) + 4):
            protected.add(index)
            indented.append(index)
            continue
        item = ITEM.match(line)
        if item:
            list_indent = len(item[1]) + len(item[2]) + len(item[3])
        elif line.strip() and not line.startswith(" "):
            list_indent = None
        if match := FENCE.match(item[4] if item else line):
            fence = match[2]
            protected.add(index)
            continue
        endings = ((r"^\s*<!--", "-->"), (r"^\s*<\?", r"\?>"),
                   (r"^\s*<!\[CDATA\[", r"\]\]>"),
                   (r"^\s*<(script|style|pre|textarea)(?:\s|>)", None))
        for opening, ending in endings:
            if match := re.search(opening, line, re.I):
                ending = ending or rf"</{match[1]}>"
                protected.add(index)
                if not re.search(ending, line[match.end():], re.I):
                    html_end = ending
                break
        else:
            if HTML.match(line) or re.fullmatch(r" {0,3}</?[A-Za-z][\w-]*(?:\s+[^<>]*)?/?>\s*", line):
                protected.add(index)
                html_end = "blank"
    for left, right in zip(indented, indented[1:]):
        if all(not lines[i].strip() for i in range(left + 1, right)):
            protected.update(range(left + 1, right))
    return protected


def check_content(data: bytes) -> list[tuple[int, str]]:
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        return [(1, "use UTF-8 text")]
    if text.startswith("<!-- Generated"):
        return []
    issues = []
    if "\r" in text:
        issues.append((1, "use LF line endings"))
    if not text.endswith("\n") or text.endswith("\n\n"):
        issues.append((max(1, len(text.splitlines())), "end with exactly one newline"))
    lines = text.replace("\r\n", "\n").split("\n")
    if lines[-1] == "":
        lines.pop()
    protected = literal_lines(lines)
    tables = set()
    starts, ends = set(), set()
    for index in range(1, len(lines)):
        if index in protected or index - 1 in protected or not separator(lines[index]):
            continue
        start, end = index - 1, index + 1
        while end < len(lines) and lines[end].strip() and "|" in lines[end] and end not in protected:
            end += 1
        starts.add(start)
        ends.add(end - 1)
        count = len(cells(lines[start].strip()))
        for row_index in range(start, end):
            tables.add(row_index)
            row = cells(lines[row_index].strip())
            canonical = [c.strip() for c in row]
            if row_index == index:
                canonical = [(":" if c.startswith(":") else "") + "---" +
                             (":" if c.endswith(":") else "") for c in canonical]
            if lines[row_index] != "| " + " | ".join(canonical) + " |":
                issues.append((row_index + 1, "use '| cell | cell |' and three-hyphen separators"))
            if len(row) != count:
                issues.append((row_index + 1, "keep the same number of table cells in every row"))

    paragraph = []

    def check_width():
        if any(re.search(r"!\[| {2}$|\\$", lines[i]) for i in paragraph):
            return
        for i in paragraph:
            line = lines[i]
            if len(line) <= 100:
                continue
            content = ITEM.sub(lambda m: " " * (len(m[1]) + len(m[2]) + len(m[3])) + m[4], line)
            words = re.findall(r"(`+)(.*?)\1|(\S+)", content)
            widths = [len(a) * 2 + len(b) if a else len(c) for a, b, c in words]
            indent = len(content) - len(content.lstrip())
            if len(widths) != 1 or widths[0] + indent <= 100:
                issues.append((i + 1, "wrap prose at 100 columns (keep indivisible tokens intact)"))

    list_indents = []
    for index, line in enumerate(lines):
        if index in protected:
            check_width()
            paragraph = []
            # Top-level fenced blocks must be separated from surrounding prose.
            if FENCE.match(line) and not line.startswith(" "):
                if index == 0 or index - 1 not in protected:
                    starts.add(index)
                if index + 1 == len(lines) or index + 1 not in protected:
                    ends.add(index)
            continue
        if line.rstrip() != line:
            trailing = line[len(line.rstrip()):]
            if not line.strip() or trailing != "  ":
                issues.append((index + 1, "remove trailing whitespace (two-space hard breaks are allowed)"))
        if not line.strip() and index and not lines[index - 1].strip() and index - 1 not in protected:
            issues.append((index + 1, "use a single blank line between blocks"))
        heading = bool(HEADING.match(line))
        if re.match(r"^ {0,3}#{1,6}[^#\s]", line) and not (
                index and lines[index - 1].strip() and re.match(r"^\s*#\d+\]", line)):
            issues.append((index + 1, "put a space after the heading marker"))
        if heading:
            starts.add(index)
            ends.add(index)
        # Setext headings are excluded inside tables and horizontal rules remain valid.
        if index and index not in tables and re.fullmatch(r" {0,3}(?:=+|-+)\s*", line) and lines[index - 1].strip() and index - 1 not in protected:
            issues.append((index + 1, "use ATX headings (#, ##, ###)"))
        item = ITEM.match(line)
        if re.fullmatch(r"\s*(?:\*\s*){3,}", line):
            item = None
        if item:
            check_width()
            paragraph = []
            if item[2] in ("+", "*"):
                issues.append((index + 1, "use '- ' for unordered lists"))
            if item[2].endswith(")"):
                issues.append((index + 1, "use '1.' style ordered list markers"))
            if not list_indents:
                starts.add(index)
            marker_indent = len(item[1])
            while list_indents and marker_indent <= list_indents[-1][0]:
                list_indents.pop()
            list_indents.append((marker_indent, marker_indent + len(item[2]) + len(item[3])))
        elif line.strip() and list_indents:
            indent = len(line) - len(line.lstrip(" "))
            if index and not lines[index - 1].strip():
                while list_indents and indent <= list_indents[-1][0]:
                    list_indents.pop()
            if list_indents and indent == 0:
                if index and lines[index - 1].strip():
                    issues.append((index + 1, "separate a top-level list from following prose"))
                list_indents = []
            elif list_indents and indent not in (content for _, content in list_indents):
                issues.append((index + 1, "align list continuation text with the item's text"))
        if not line.strip() or heading or index in tables or line.lstrip().startswith(">"):
            check_width()
            paragraph = []
        else:
            paragraph.append(index)
    check_width()
    for index in sorted(starts):
        if index and lines[index - 1].strip():
            issues.append((index + 1, "add a blank line before this block"))
    for index in sorted(ends):
        if index + 1 < len(lines) and lines[index + 1].strip():
            issues.append((index + 1, "add a blank line after this block"))
    return sorted(set(issues))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT, help="repository root (default: this repository)")
    args = parser.parse_args(argv)
    root = args.root.resolve()
    files = markdown_files(root)
    if not files:
        print(f"{root}: no Markdown files found", file=sys.stderr)
        return 1
    failed = 0
    for path in files:
        try:
            issues = check_content(path.read_bytes())
        except OSError as error:
            issues = [(1, f"cannot read file: {error}")]
        for line, message in issues:
            print(f"{path.relative_to(root)}:{line}: {message}", file=sys.stderr)
        failed += bool(issues)
    if failed:
        print(f"Markdown: {failed}/{len(files)} files failed; see docs/markdown_style.md", file=sys.stderr)
        return 1
    print(f"Markdown: {len(files)} files checked (generated files exempt)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
