#!/usr/bin/env python3
"""Stage, never install/execute, the upstream macOS arm64 core pinned for this build."""
import argparse
import hashlib
import os
from pathlib import Path
import tempfile
import urllib.request

NAME = 'fury-core-0.5.7-macos-arm64.tar.xz'
URL = 'https://github.com/furyteamtop/fury-antidetect-browser/releases/download/v0.5.7/' + NAME
SHA256 = '148dbb1560f7b7e4ed9e0ffc34734597a197c2df3e3c54f3c4ddab35ee1d299a'
SIZE = 107098824

def verify(path):
    if path.stat().st_size != SIZE:
        raise ValueError('Core size mismatch; refusing this file')
    h = hashlib.sha256()
    with path.open('rb') as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b''):
            h.update(chunk)
    if h.hexdigest() != SHA256:
        raise ValueError('Core SHA256 mismatch; refusing this file')

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output-dir', type=Path, default=Path('personal-artifacts'))
    p.add_argument('--verify-only', type=Path, help='Verify an existing archive without network or writes')
    a = p.parse_args()
    if a.verify_only:
        verify(a.verify_only)
        print('Verified pinned upstream core SHA256:', SHA256)
        return
    a.output_dir.mkdir(parents=True, exist_ok=True)
    dest = a.output_dir / NAME
    if dest.exists():
        verify(dest)  # Never overwrite an existing mismatched file silently.
    else:
        fd, temp = tempfile.mkstemp(prefix='.core-download-', dir=a.output_dir)
        try:
            with os.fdopen(fd, 'wb') as out, urllib.request.urlopen(URL, timeout=120) as src:
                seen = 0
                while chunk := src.read(1024 * 1024):
                    seen += len(chunk)
                    if seen > SIZE:
                        raise ValueError('Download exceeds pinned size')
                    out.write(chunk)
            verify(Path(temp))
            # link is atomic and refuses clobbering a concurrent download.
            os.link(temp, dest)
        finally:
            Path(temp).unlink(missing_ok=True)
    print(f'Staged verified upstream precompiled core: {dest}')
    print('Not installed or executed; not self-compiled; Apple signature/notarization not verified here.')

if __name__ == '__main__':
    main()
