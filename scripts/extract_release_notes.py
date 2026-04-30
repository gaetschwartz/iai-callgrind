#!/usr/bin/env python3
import re
import sys
from pathlib import Path


def extract(changelog: str, version: str) -> str:
    version_pattern = re.compile(r"^## \[" + re.escape(version) + r"\]")
    section_pattern = re.compile(r"^## \[")

    lines = changelog.splitlines()
    section_lines: list[str] = []
    found = False

    for line in lines:
        if section_pattern.match(line):
            if found:
                break
            if version_pattern.match(line):
                found = True
                section_lines.append(line)
            continue

        if not found:
            continue

        section_lines.append(line)

    if not found:
        print(
            f"Error: Version [{version}] not found",
            file=sys.stderr,
        )
        sys.exit(1)

    return reflow(section_lines)


def reflow(lines: list[str]) -> str:
    result: list[str] = []
    current = ""

    for line in lines:
        if re.match(r"^### ", line) or line == "":
            if current:
                result.append(current)
                current = ""
            result.append(line)
        elif re.match(r"^(    )?- ", line):
            if current:
                result.append(current)
                current = ""
            current = line
        elif re.match(r"^[ \t]+", line):
            current += " " + line.strip()
        elif current:
            current += " " + line
        else:
            current = line

    if current:
        result.append(current)

    return "\n".join(result)


def main() -> None:
    if len(sys.argv) < 2:
        print(
            f"Usage: {sys.argv[0]} <version> [changelog_path]",
            file=sys.stderr,
        )
        sys.exit(1)

    version = sys.argv[1].lstrip("v")
    changelog_path = Path(sys.argv[2]) if len(sys.argv) > 2 else Path("CHANGELOG.md")

    try:
        changelog = changelog_path.read_text()
    except FileNotFoundError:
        print(
            f"Error: File not found: {changelog_path}",
            file=sys.stderr,
        )
        sys.exit(1)

    print(extract(changelog, version))


if __name__ == "__main__":
    main()
