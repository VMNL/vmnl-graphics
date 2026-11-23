#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Hugo Duda
# SPDX-License-Identifier: MIT

"""Check VMNL Rustdoc comment structure or rendered example links."""

import argparse
from html.parser import HTMLParser
from itertools import groupby
from pathlib import Path
import re
import sys
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parents[1]
DOC_LINE = re.compile(r"^[ \t]*(///(?!/)|//!) ?(.*)$")
FENCE = re.compile(r"^ {0,3}(`{3,}|~{3,})[ \t]*(.*?)\s*$")
HEADING = re.compile(r"^ {0,3}#[ \t]+(.+?)(?:[ \t]+#+)?[ \t]*$")
SECTION_ORDER = ("Arguments", "Errors", "Safety", "Example")
EXAMPLE_FRAGMENT = r"example(?:-[a-z0-9-]+)?"
EXAMPLE_TOKEN = re.compile(
    rf"(?P<link>(?<![\\!])\[[^\]]+\]\((?:[\w:@]+)?#{EXAMPLE_FRAGMENT}\))"
    r"|(?<![\\`])(?P<ticks>`+)(?!`).*?(?<!`)(?P=ticks)(?!`)",
    re.DOTALL,
)
RUST_FENCE = re.compile(
    r"(?:rust|no_run|ignore(?:-[\w-]+)?|compile_fail|should_panic|edition20\d\d)"
)


def doc_blocks(source: str):
    lines = (
        (number, DOC_LINE.match(line))
        for number, line in enumerate(source.splitlines(), 1)
    )
    for marker, group in groupby(lines, key=lambda entry: entry[1][1] if entry[1] else None):
        if marker:
            yield [(number, match[2]) for number, match in group]


def markdown_parts(block):
    """Yield prose lines and complete fenced blocks with their Rustdoc language tags."""
    fence = None
    for number, line in block:
        match = FENCE.fullmatch(line)
        if fence is None:
            if match:
                fence = match
                start = number
                code = []
            else:
                yield number, line, None
        elif (
            match
            and match[1][0] == fence[1][0]
            and len(match[1]) >= len(fence[1])
            and not match[2]
        ):
            yield start, "\n".join(code), fence[2]
            fence = None
        else:
            code.append(line)


def sections(block):
    heading = None
    for number, content, language in markdown_parts(block):
        match = HEADING.fullmatch(content) if language is None else None
        if match:
            if heading:
                yield start, heading, parts
            start, heading, parts = number, match[1], []
        elif heading:
            parts.append((content, language))
    if heading:
        yield start, heading, parts


def has_example(parts) -> bool:
    prose = []
    for content, language in parts:
        if language is None:
            prose.append(content)
        else:
            tags = re.split(r"[,\s]+", language) if language else []
            if content.strip() and all(RUST_FENCE.fullmatch(tag) for tag in tags):
                return True
            prose.append("")
    return any(token["link"] for token in EXAMPLE_TOKEN.finditer("\n".join(prose)))


def source_errors(path: Path, source: str) -> list[str]:
    errors = []
    for block in doc_blocks(source):
        seen = set()
        last_rank = -1
        for line, heading, parts in sections(block):
            location = f"{path}:{line}"
            if heading.casefold() == "panics":
                errors.append(f"{location}: # Panics is forbidden in library Rustdoc")
            if heading not in SECTION_ORDER:
                continue
            rank = SECTION_ORDER.index(heading)
            if heading in seen:
                errors.append(f"{location}: duplicate # {heading} section")
            elif rank < last_rank:
                errors.append(f"{location}: # {heading} section is out of order")
            seen.add(heading)
            last_rank = max(last_rank, rank)
            if not any(content.strip() for content, _ in parts):
                errors.append(f"{location}: empty # {heading} section")
            elif heading == "Example" and not has_example(parts):
                errors.append(f"{location}: # Example needs Rust code or a direct example link")
    return errors


def check_sources(root: Path) -> list[str]:
    errors = []
    for path in sorted(root.glob("crates/*/src/**/*.rs")):
        errors.extend(source_errors(path.relative_to(root), path.read_text(encoding="utf-8")))
    return errors


class RustdocPage(HTMLParser):
    def __init__(self, path: Path):
        super().__init__()
        self.ids: set[str] = set()
        self.example_links: list[tuple[int, str]] = []
        self.feed(path.read_text(encoding="utf-8"))
        self.close()

    def handle_starttag(self, tag, attrs):
        attributes = dict(attrs)
        anchor = attributes.get("id")
        if anchor:
            self.ids.add(anchor)
        href = attributes.get("href")
        if tag == "a" and href:
            target = urlsplit(href)
            if not target.scheme and not target.netloc and re.fullmatch(
                EXAMPLE_FRAGMENT, unquote(target.fragment)
            ):
                self.example_links.append((self.getpos()[0], href))


def check_rendered_examples(crate_docs: Path) -> list[str]:
    crate_docs = crate_docs.resolve()
    doc_root = crate_docs.parent
    files = sorted(crate_docs.rglob("*.html"))
    if not files:
        return [f"{crate_docs}: missing Rustdoc pages"]
    pages = {}
    errors = []
    for path in files:
        if path not in pages:
            pages[path] = RustdocPage(path)
        for line, href in pages[path].example_links:
            target = urlsplit(href)
            target_path = unquote(target.path)
            if target_path.startswith("/"):
                destination = (doc_root / target_path.lstrip("/")).resolve()
            else:
                destination = (path.parent / target_path).resolve() if target_path else path
            location = f"{path.relative_to(doc_root)}:{line}"
            if not destination.is_relative_to(doc_root) or not destination.is_file():
                errors.append(f"{location}: missing Rustdoc page for example link {href}")
                continue
            if destination not in pages:
                pages[destination] = RustdocPage(destination)
            if unquote(target.fragment) not in pages[destination].ids:
                errors.append(f"{location}: missing Rustdoc anchor for example link {href}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--rustdoc-dir", type=Path, help="check example links in a built crate's Rustdoc HTML",
    )
    args = parser.parse_args()
    try:
        if args.rustdoc_dir is None:
            errors = check_sources(ROOT)
        else:
            errors = check_rendered_examples(args.rustdoc_dir)
    except (OSError, UnicodeError) as error:
        print(f"check-comments: {error}", file=sys.stderr)
        return 1
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    if args.rustdoc_dir is None:
        print("Rustdoc comment structure checked")
    else:
        print("Rustdoc example links checked")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
