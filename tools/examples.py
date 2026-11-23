#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 VMNL contributors
# SPDX-License-Identifier: MIT

"""Create a runnable VMNL example or check the workspace and example catalogue."""

import argparse
from datetime import date
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
DOMAINS = ("d2", "input", "raw", "window")
NAME = re.compile(r"[a-z][a-z0-9]*(?:_[a-z0-9]+)*")
CATALOGUE_HEADER = "| Example | Command | Covers |"
ROW = re.compile(r"\| \[`([^\x60]+)`\]\(([^)]+)\) \| `just run ([^\x60]+)` \| (.+) \|")


def example_members(manifest):
    return [
        path
        for path in tomllib.loads(manifest)["workspace"]["members"]
        if path.startswith("examples/")
    ]


def catalogue_lines(catalogue):
    lines = catalogue.splitlines(keepends=True)
    header = lines.index(CATALOGUE_HEADER + "\n")
    start = header + 2
    end = start
    while end < len(lines) and lines[end].startswith("|"):
        end += 1
    return lines, start, end


def missing_binary_features(package, binary):
    features = package["features"]
    required = set(binary.get("required-features", ())) & features.keys()
    if not required:
        return []
    enabled = set()
    pending = ["default"] if "default" in features else []
    while pending:
        feature = pending.pop()
        if feature not in enabled:
            enabled.add(feature)
            pending.extend(features.get(feature, ()))
    return sorted(required - enabled)


def check_cargo(root, packages):
    result = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--offline", "--format-version=1"],
        cwd=root,
        check=False,
        capture_output=True,
        encoding="utf-8",
    )
    if result.returncode:
        return [f"Cargo could not load the workspace:\n{result.stderr.rstrip()}"]

    lockfile = root / "Cargo.lock"
    locked = set()
    if lockfile.is_file():
        locked = {
            (package["name"], package["version"])
            for package in tomllib.loads(lockfile.read_text(encoding="utf-8"))["package"]
            if "source" not in package
        }

    errors = []
    unlocked = []
    workspace = json.loads(result.stdout)
    members = set(workspace["workspace_members"])
    cargo_packages = {
        package["name"]: package
        for package in workspace["packages"]
        if package["id"] in members
    }
    for name, path in packages.items():
        package = cargo_packages.get(name)
        if package is None:
            errors.append(f"{name}: Cargo does not list this workspace package")
            continue
        binaries = [target for target in package["targets"] if target["kind"] == ["bin"]]
        if package["default_run"] is not None:
            binaries = [target for target in binaries if target["name"] == package["default_run"]]
        expected_source = (root / path / "src/main.rs").resolve()
        if len(binaries) != 1 or Path(binaries[0]["src_path"]).resolve() != expected_source:
            errors.append(f"{name}: cargo run must select the linked src/main.rs binary")
        else:
            missing = missing_binary_features(package, binaries[0])
            if missing:
                errors.append(f"{name}: cargo run requires features enabled by default: {', '.join(missing)}")
        if (name, package["version"]) not in locked:
            unlocked.append(name)
    if unlocked:
        errors.append(f"Cargo.lock is missing {', '.join(sorted(unlocked))}; run cargo update --workspace")
    return errors


def check(root):
    errors = []
    members = example_members((root / "Cargo.toml").read_text(encoding="utf-8"))
    packages = {}
    directories = {
        path.parent.relative_to(root).as_posix()
        for path in (root / "examples").rglob("Cargo.toml")
    }
    for path in sorted(directories - set(members)):
        errors.append(f"{path}: missing workspace member")
    if len(members) != len(set(members)):
        errors.append("duplicate example workspace member")
    for path in members:
        if path not in directories:
            errors.append(f"{path}: missing Cargo.toml")
            continue
        manifest = tomllib.loads((root / path / "Cargo.toml").read_text(encoding="utf-8"))
        package = manifest["package"]["name"]
        if package in packages:
            errors.append(f"duplicate example package: {package}")
        packages[package] = path

    catalogue = (root / "examples/README.md").read_text(encoding="utf-8")
    lines, start, end = catalogue_lines(catalogue)
    seen = set()
    for line in lines[start:end]:
        match = ROW.fullmatch(line.rstrip("\n"))
        if match is None:
            errors.append(f"invalid catalogue row: {line.strip()}")
            continue
        name, source, command, _description = match.groups()
        if name in seen:
            errors.append(f"duplicate catalogue entry: {name}")
        seen.add(name)
        path = packages.get(name)
        if path is None:
            errors.append(f"{name}: catalogue entry has no workspace package")
        elif f"examples/{source}" != f"{path}/src/main.rs" or not (root / path / "src/main.rs").is_file():
            errors.append(f"{name}: missing or incorrect source link")
        if command != name:
            errors.append(f"{name}: incorrect run command")
    for name in sorted(packages.keys() - seen):
        errors.append(f"{name}: missing catalogue entry")
    return errors or check_cargo(root, packages)


def author_name(root, author):
    if author is None:
        result = subprocess.run(
            ["git", "config", "--get", "user.name"],
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        )
        author = result.stdout.strip()
    if not author.strip() or len(author.splitlines()) != 1:
        raise ValueError("set git user.name or pass a single-line --author")
    return author.strip()


def replace_text(file, content):
    with tempfile.TemporaryDirectory(dir=file.parent) as staging:
        replacement = Path(staging) / file.name
        replacement.write_text(content, encoding="utf-8")
        replacement.chmod(file.stat().st_mode & 0o777)
        replacement.replace(file)


def create(root, domain, name, dry_run, author):
    if NAME.fullmatch(name) is None:
        raise ValueError("example name must use ASCII snake_case")
    package = f"{domain}_{name}"
    path = f"examples/{domain}/{name}"
    directory = root / path
    manifest_file = root / "Cargo.toml"
    catalogue_file = root / "examples/README.md"
    manifest = manifest_file.read_text(encoding="utf-8")
    catalogue = catalogue_file.read_text(encoding="utf-8")
    members = tomllib.loads(manifest)["workspace"]["members"]
    if directory.exists() or directory.is_symlink() or path in members:
        raise ValueError(f"example path already exists: {path}")
    for member in members:
        member_file = root / member / "Cargo.toml"
        if member_file.is_file():
            data = tomllib.loads(member_file.read_text(encoding="utf-8"))
            if data.get("package", {}).get("name") == package:
                raise ValueError(f"workspace package already exists: {package}")
    lines, start, end = catalogue_lines(catalogue)
    if any(line.startswith(f"| [`{package}`]") for line in lines[start:end]):
        raise ValueError(f"catalogue entry already exists: {package}")

    members_block = re.search(r"(?m)^members = \[\n(?:[^\n]*\n)*?^\]", manifest)
    if members_block is None:
        raise ValueError("expected a multiline workspace members list")
    example_lines = list(re.finditer(r'(?m)^    "(examples/[^"]+)",\n', members_block.group()))
    later_member = next((entry for entry in example_lines if entry[1] > path), None)
    if later_member is not None:
        position = members_block.start() + later_member.start()
    elif example_lines:
        position = members_block.start() + example_lines[-1].end()
    else:
        position = members_block.end() - 1
    updated_manifest = manifest[:position] + f'    "{path}",\n' + manifest[position:]
    if example_members(updated_manifest).count(path) != 1:
        raise ValueError("failed to insert package in workspace members")

    template = "raw" if domain == "raw" else "window"
    if dry_run:
        print(f"Create {path}/Cargo.toml and {path}/src/main.rs ({template} template)")
        print(f"Register {path} in Cargo.toml and {package} in examples/README.md")
        return

    source = (ROOT / "tools/example_templates" / f"{template}.rs.in").read_text(encoding="utf-8")
    source = (
        source.replace("@PACKAGE@", package)
        .replace("@YEAR@", str(date.today().year))
        .replace("@AUTHOR@", author_name(root, author))
    )
    description = (
        "Draw a colored triangle using a raw pipeline." if domain == "raw"
        else "Draw a cyan rectangle."
    )
    row_index = end
    for index in range(start, end):
        match = ROW.fullmatch(lines[index].rstrip("\n"))
        if match and match[1] > package:
            row_index = index
            break
    lines.insert(
        row_index,
        f"| [`{package}`]({domain}/{name}/src/main.rs) | `just run {package}` | {description} |\n",
    )
    updated_catalogue = "".join(lines)
    new_manifest = (
        f'[package]\nname = "{package}"\nversion.workspace = true\n'
        'edition.workspace = true\npublish = false\n\n[dependencies]\n'
        'vmnl_graphics = { path = "../../../crates/vmnl_graphics" }\n'
    )

    # Reserve the destination before changing any shared files.
    directory.mkdir()
    changed = []
    try:
        (directory / "src").mkdir()
        (directory / "Cargo.toml").write_text(new_manifest, encoding="utf-8")
        (directory / "src/main.rs").write_text(source, encoding="utf-8")
        for file, original, updated in (
            (manifest_file, manifest, updated_manifest),
            (catalogue_file, catalogue, updated_catalogue),
        ):
            if file.read_text(encoding="utf-8") != original:
                raise ValueError(f"{file} changed during example creation")
            changed.append((file, original, updated))
            replace_text(file, updated)
    except (OSError, ValueError, KeyboardInterrupt) as error:
        conflicts = []
        for file, original, updated in reversed(changed):
            try:
                current = file.read_text(encoding="utf-8")
                if current == updated:
                    replace_text(file, original)
                elif current != original:
                    conflicts.append(f"{file.relative_to(root)} changed during rollback")
            except (OSError, ValueError) as rollback_error:
                conflicts.append(f"{file.relative_to(root)}: {rollback_error}")
        if conflicts:
            raise ValueError(
                f"{error}; rollback incomplete ({'; '.join(conflicts)}). "
                f"Kept {path} for recovery. Review the manifest and catalogue before retrying."
            ) from error
        (directory / "src/main.rs").unlink(missing_ok=True)
        if (directory / "src").is_dir():
            (directory / "src").rmdir()
        (directory / "Cargo.toml").unlink(missing_ok=True)
        directory.rmdir()
        raise
    print(f"Created {package}. Run cargo update --workspace, then just build {package}.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("check")
    new = commands.add_parser("new")
    new.add_argument("domain", choices=DOMAINS)
    new.add_argument("name")
    new.add_argument("--author")
    new.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    try:
        if args.command == "check":
            errors = check(ROOT)
            if errors:
                print("\n".join(errors), file=sys.stderr)
                return 1
            print("Example catalogue matches the workspace.")
        elif args.dry_run:
            create(ROOT, args.domain, args.name, True, args.author)
        else:
            lock = ROOT / "examples/.example-new.lock"
            lock.mkdir()
            try:
                create(ROOT, args.domain, args.name, False, args.author)
            finally:
                lock.rmdir()
    except (OSError, ValueError, KeyError) as error:
        print(f"examples: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
