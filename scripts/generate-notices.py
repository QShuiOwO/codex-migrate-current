"""Collect locked Windows dependency licenses without accessing migration data."""

import argparse
import json
import pathlib
import re
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', default='x86_64-pc-windows-msvc')
    parser.add_argument('--metadata', type=pathlib.Path)
    parser.add_argument('--output', type=pathlib.Path, default=pathlib.Path('dist/notices'))
    args = parser.parse_args()
    root = pathlib.Path(__file__).resolve().parents[1]
    if args.metadata:
        metadata = json.loads(args.metadata.read_text(encoding='utf-8-sig'))
    else:
        metadata = json.loads(subprocess.check_output([
            'cargo', 'metadata', '--locked', '--format-version', '1',
            '--filter-platform', args.target, '--features', 'gui',
        ], cwd=root, text=True, encoding='utf-8'))
    packages = {p['id']: p for p in metadata['packages']}
    nodes = {n['id']: n for n in metadata['resolve']['nodes']}
    seen = set()
    pending = [metadata['resolve']['root']]
    while pending:
        key = pending.pop()
        if key in seen:
            continue
        seen.add(key)
        pending.extend(d['pkg'] for d in nodes[key]['deps']
                       if any(k['kind'] != 'dev' for k in d['dep_kinds']))
    supplied_root = root / 'docs/third-party'
    supplied = json.loads((supplied_root / 'sources.json').read_text(encoding='utf-8'))
    sections = [
        'Third-party notices for Codex Migrate Current',
        f'Target: {args.target}; feature: gui; dependencies locked by Cargo.lock.',
        'This inventory includes normal/build dependencies resolved by Cargo; '
        'some optional packages may not be linked. Development-only packages are excluded.',
    ]
    inventory = []
    missing = []
    for key in sorted(seen, key=lambda k: (packages[k]['name'], packages[k]['version'])):
        p = packages[key]
        if not p['source']:
            continue
        crate_root = pathlib.Path(p['manifest_path']).parent
        files = sorted(f for f in crate_root.rglob('*') if f.is_file() and (
            f.name.lower().startswith(('license', 'licence', 'copying', 'notice', 'copyright'))
            or (p['name'] == 'epaint_default_fonts' and f.suffix.lower() == '.txt')
        ))
        if p['license_file']:
            license_path = crate_root / p['license_file']
            if license_path.is_file() and license_path not in files:
                files.append(license_path)
        extra = supplied.get(p['name'] + '@' + p['version'], {})
        texts = [(f.relative_to(crate_root).as_posix(),
                  f.read_text(encoding='utf-8', errors='replace')) for f in files]
        texts.extend((name, (supplied_root / name).read_text(encoding='utf-8'))
                     for name in extra.get('files', []))
        if p['name'] == 'epaint_default_fonts':
            texts.append(('README.md', (crate_root / 'README.md').read_text(encoding='utf-8')))
        if p['name'] == 'libsqlite3-sys':
            sqlite = crate_root / 'sqlite3/sqlite3.c'
            if not sqlite.is_file():
                raise RuntimeError('Bundled SQLite source dedication is missing')
            comments = re.findall(r'/\*.*?\*/', sqlite.read_text(encoding='utf-8', errors='replace')[:10000], re.S)
            header = next((text for text in comments if 'disclaims copyright' in text.lower()), None)
            if header is None:
                raise RuntimeError('Bundled SQLite dedication was not recognized')
            texts.append(('sqlite3/sqlite3.c: public-domain dedication', header))
        if not texts:
            missing.append(p['name'] + '@' + p['version'])
            continue
        entry = {
            'name': p['name'], 'version': p['version'], 'license': p['license'],
            'authors': p['authors'], 'repository': p['repository'],
            'license_files': [name for name, _ in texts],
            'upstream_license_sources': extra.get('urls', []),
        }
        inventory.append(entry)
        sections.extend([
            '\n' + '=' * 78,
            f"{p['name']} {p['version']} — {p['license']}",
            'Authors: ' + ', '.join(p['authors']),
            'Repository: ' + (p['repository'] or 'https://crates.io/crates/' + p['name']),
        ])
        for name, content in texts:
            sections.extend(['\n--- ' + name + ' ---', content.rstrip()])
    if missing:
        raise RuntimeError('Missing complete license texts: ' + ', '.join(missing))
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / 'THIRD_PARTY_NOTICES.txt').write_text('\n'.join(sections) + '\n', encoding='utf-8')
    (args.output / 'THIRD_PARTY_COMPONENTS.json').write_text(
        json.dumps({'target': args.target, 'features': ['gui'], 'packages': inventory},
                   indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    print(f'Collected complete license notices for {len(inventory)} dependency packages.')


if __name__ == '__main__':
    main()
