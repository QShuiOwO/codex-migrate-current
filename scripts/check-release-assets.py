"""Require both platform packages at the Cargo version before releasing."""
import argparse
import hashlib
from pathlib import Path
import re


def verify(directory, version):
    expected = set()
    for platform in ('Windows-x64.zip', 'Ubuntu-24.04-x64.tar.gz'):
        name = f'Codex-Migrate-Current-{version}-{platform}'
        archive = directory / name
        checksum = directory / (name + '.sha256')
        expected.update((name, name + '.sha256'))
        if not archive.is_file() or not checksum.is_file():
            raise ValueError(f'Missing release archive or checksum: {name}')
        fields = checksum.read_text(encoding='utf-8-sig').strip().split()
        actual = hashlib.sha256(archive.read_bytes()).hexdigest()
        if fields != [actual, name]:
            raise ValueError(f'Invalid checksum or filename: {checksum.name}')
    if {p.name for p in directory.iterdir()} != expected:
        raise ValueError('Unexpected release assets; expected only two archives and their checksums')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    args = parser.parse_args()
    manifest = Path(__file__).resolve().parents[1] / 'Cargo.toml'
    version = re.search(r'^version = "([^"]+)"', manifest.read_text(), re.M)[1]
    verify(args.directory, version)
    print(f'Verified Windows and Ubuntu {version} release archives and SHA-256 checksums')


if __name__ == '__main__':
    main()
