#!/usr/bin/env python3
"""Add or remove only OmacDiag's row in the Omarchy user menu extension."""

import argparse
import json
import os
import re
import shutil
import stat
import tempfile
import time
from pathlib import Path

ROW_ID = "trigger.hardware.omacdiag"
ROW = (
    '  "trigger.hardware.omacdiag": '
    '{"icon":"󰒋","label":"OmacDiag","description":"Intel Mac hardware diagnostics",'
    '"action":"omarchy-shell shell summon io.github.sonique6784.omacdiag \'{}\'"}'
)
BEGIN = "  // BEGIN OMACDIAG MENU ENTRY"
END = "  // END OMACDIAG MENU ENTRY"


def without_comments(value: str) -> str:
    return "".join(
        re.sub(r"[^\n]", " ", line) if line.lstrip().startswith("//") else line
        for line in value.splitlines(keepends=True)
    )


def parse_jsonc(value: str) -> dict:
    source = without_comments(value)
    output = []
    quoted = False
    escaped = False
    for index, char in enumerate(source):
        if quoted:
            output.append(char)
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                quoted = False
        elif char == '"':
            quoted = True
            output.append(char)
        elif char == "," and re.match(r"\s*[}\]]", source[index + 1 :]):
            output.append(" ")
        else:
            output.append(char)
    parsed = json.loads("".join(output))
    if not isinstance(parsed, dict):
        raise ValueError("menu extension must contain a JSON object")
    return parsed


def install(source: str) -> str:
    data = parse_jsonc(source)
    if ROW_ID in data or BEGIN in source or END in source:
        raise ValueError("OmacDiag menu entry already exists or conflicts with an existing row")
    bare = without_comments(source)
    close = len(bare.rstrip()) - 1
    if close < 0 or bare[close] != "}":
        raise ValueError("menu extension must end with a JSON object")
    previous = bare[:close].rstrip()[-1]
    comma = "" if previous in "{," else ","
    block = f"{comma}\n{BEGIN}\n{ROW},\n{END}\n"
    updated = source[:close] + block + source[close:]
    parse_jsonc(updated)
    return updated


def remove(source: str) -> str:
    for comma in (",", ""):
        block = f"{comma}\n{BEGIN}\n{ROW},\n{END}\n"
        if source.count(block) == 1:
            updated = source.replace(block, "", 1)
            parse_jsonc(updated)
            return updated
    raise ValueError("owned OmacDiag menu block was not found unchanged; edit it manually")


def write_atomically(path: Path, updated: str) -> None:
    mode = stat.S_IMODE(path.stat().st_mode) if path.exists() else 0o644
    if path.exists():
        backup = path.with_name(path.name + f".bak.{time.time_ns()}")
        with path.open("rb") as source, backup.open("xb") as target:
            shutil.copyfileobj(source, target)
        backup.chmod(mode)
        print(f"Backup: {backup}")
    fd, temp_name = tempfile.mkstemp(prefix=".omacdiag-menu-", dir=path.parent)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as temp:
            temp.write(updated)
            temp.flush()
            os.fsync(temp.fileno())
        os.chmod(temp_name, mode)
        os.replace(temp_name, path)
    finally:
        if os.path.exists(temp_name):
            os.unlink(temp_name)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("install", "remove"))
    parser.add_argument(
        "--file",
        type=Path,
        default=Path.home() / ".config/omarchy/extensions/omarchy-menu.jsonc",
        help="menu extension path (mainly for testing)",
    )
    args = parser.parse_args()
    path = args.file
    if path.is_symlink():
        parser.error("refusing to edit a symlink")
    if not path.parent.is_dir():
        parser.error(f"menu directory does not exist: {path.parent}")
    if path.exists() and not path.is_file():
        parser.error("menu path is not a regular file")
    original = path.read_text(encoding="utf-8") if path.exists() else "{}\n"
    try:
        updated = install(original) if args.action == "install" else remove(original)
    except (ValueError, json.JSONDecodeError) as error:
        parser.error(str(error))
    write_atomically(path, updated)
    print(f"OmacDiag menu entry {'installed' if args.action == 'install' else 'removed'}: {path}")


if __name__ == "__main__":
    main()
