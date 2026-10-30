"""Package locally built Ubuntu binaries and licenses; does not install or publish."""
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tarfile
import tempfile


def main():
    root = Path(__file__).resolve().parents[1]
    if platform.system() != 'Linux' or platform.machine() != 'x86_64':
        raise SystemExit('This package targets Ubuntu 24.04 x64; build on that platform.')
    version = re.search(r'^version = "([^"]+)"', (root / 'Cargo.toml').read_text(), re.M)[1]
    name = f'Codex-Migrate-Current-{version}-Ubuntu-24.04-x64'
    output = root / 'dist/release'
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='linux-package-', dir=output) as staging:
        package = Path(staging) / name
        package.mkdir()
        for binary in ('codex-migrate', 'codex-migrate-gui'):
            source = root / 'target/release' / binary
            if not source.is_file() or not os.access(source, os.X_OK):
                raise SystemExit(f'Missing binary: {source}; run cargo build --locked --release --features gui --bins')
            shutil.copy2(source, package / binary)
        for document in ('LICENSE', 'README.md', 'README.zh-CN.md', 'CURRENT-VERSION.md', 'CHANGELOG.md', 'CONTRIBUTING.md', 'SECURITY.md', 'TRADEMARKS.md', 'THIRD_PARTY_NOTICES.md', 'UPSTREAM.json'):
            shutil.copy2(root / document, package / document)
        for document in ('THIRD_PARTY_NOTICES.txt', 'THIRD_PARTY_COMPONENTS.json'):
            shutil.copy2(root / 'dist/notices-linux' / document, package / document)
        (package / 'docs').mkdir()
        for document in ('ubuntu-cli.md', 'ubuntu-validation.md', 'current-validation.md', 'current-runtime-results.json', 'publishing.md'):
            shutil.copy2(root / 'docs' / document, package / 'docs' / document)
        shutil.copytree(root / 'docs/third-party', package / 'docs/third-party')
        rustc = shutil.which('rustc') or str(Path.home() / '.cargo/bin/rustc')
        toolchain = Path(subprocess.check_output([rustc, '--print', 'sysroot'], text=True).strip())
        rust_notices = package / 'rust-toolchain-notices'
        rust_notices.mkdir()
        for source in (toolchain / 'share/doc/rust').iterdir():
            if source.is_file() and source.name.startswith(('COPYRIGHT', 'LICENSE')):
                shutil.copy2(source, rust_notices / source.name)
        build = {
            'version': version,
            'commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(),
            'dirty': bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=root, text=True)),
            'platform': platform.platform(),
            'rust': subprocess.check_output([rustc, '--version'], text=True).strip(),
        }
        (package / 'BUILD-INFO.json').write_text(json.dumps(build, indent=2) + '\n')
        checksums = [f'{hashlib.sha256((package / b).read_bytes()).hexdigest()}  {b}' for b in ('codex-migrate', 'codex-migrate-gui')]
        (package / 'SHA256SUMS.txt').write_text('\n'.join(checksums) + '\n')
        archive = output / (name + '.tar.gz')
        with tarfile.open(archive, 'w:gz') as tar:
            tar.add(package, arcname=name)
        checksum = archive.with_name(archive.name + '.sha256')
        checksum.write_text(f'{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n')
        print(archive)
        print(checksum)


if __name__ == '__main__':
    main()
