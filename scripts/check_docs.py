#!/usr/bin/env python3
"""Offline checks for the repository documentation."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

MD_FILES = [
    ROOT / "README.md",
    ROOT / "AGENTS.md",
    ROOT / "THIRD_PARTY_NOTICES.md",
    ROOT / "src" / "README.md",
    *sorted((ROOT / "docs").glob("*.md")),
]

JSON_FILES = [
    ROOT / "src" / "package.json",
    ROOT / "src" / "ui" / "tsconfig.json",
    ROOT / "src" / "src-tauri" / "tauri.conf.json",
    ROOT / "src" / "src-tauri" / "capabilities" / "default.json",
]

LINK_RE = re.compile(r"\]\(([^)#]+)\)")

errors: list[str] = []

for path in MD_FILES:
    if not path.exists():
        errors.append(f"{path.relative_to(ROOT)}: arquivo ausente")
        continue

    text = path.read_text(encoding="utf-8")
    if text.count("```") % 2:
        errors.append(f"{path.relative_to(ROOT)}: code fences desbalanceados")

    for match in LINK_RE.finditer(text):
        link = match.group(1).strip()
        if link.startswith(("http://", "https://", "mailto:")):
            continue
        target = (path.parent / link).resolve()
        if not target.exists():
            errors.append(f"{path.relative_to(ROOT)}: link quebrado -> {link}")

for path in JSON_FILES:
    if not path.exists():
        errors.append(f"{path.relative_to(ROOT)}: arquivo ausente")
        continue
    try:
        json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        errors.append(f"{path.relative_to(ROOT)}: JSON inválido ({exc})")

if errors:
    print("\n".join(errors))
    sys.exit(1)

print(f"docs ok ({len(MD_FILES)} markdown, {len(JSON_FILES)} json)")
