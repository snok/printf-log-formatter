#!/usr/bin/env python3
import os
import re
import sys
from pathlib import Path


def main() -> int:
    if not (cargo_path := Path(os.getenv('CARGO_PATH', 'Cargo.toml'))).is_file():
        print(f'✖ path "{cargo_path}" does not exist')
        return 1

    if len(sys.argv) == 2:
        version = sys.argv[1]
        print(f'Found input version {version}')
    elif version := os.getenv('VERSION'):
        print(f'Found $VERSION {version}')
    elif version := os.getenv('GITHUB_REF'):
        print(f'Found $GITHUB_REF {version}')
    else:
        print(f'✖ "Version env variables not found')
        return 1

    version = version.lower().replace('refs/tags/v', '').replace('a', '-alpha').replace('b', '-beta')
    print(f'writing version "{version}", to {cargo_path}')

    version_regex = re.compile('^version ?= ?".*"', re.M)
    cargo_content = cargo_path.read_text()
    if not version_regex.search(cargo_content):
        print(f'✖ {version_regex!r} not found in {cargo_path}')
        return 1

    new_content = version_regex.sub(f'version = "{version}"', cargo_content)
    cargo_path.write_text(new_content)
    return 0


if __name__ == '__main__':
    sys.exit(main())
