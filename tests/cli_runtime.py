"""Windows backup -> Linux CLI -> portable backup -> Desktop, synthetic data only.

Uses a real installed Codex runtime; never reads personal history or sends a model request.
python tests/cli_runtime.py --codex-bin /path/to/codex --migrate-bin target/debug/codex-migrate
"""
from __future__ import annotations

import argparse
import datetime
import json
import os
from pathlib import Path
import sqlite3
import subprocess
from contextlib import closing

from current_runtime import Server, fixture, logical_snapshot


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--codex-bin', type=Path, required=True)
    parser.add_argument('--migrate-bin', type=Path, required=True)
    parser.add_argument('--output-parent', type=Path, default=Path('test-results'))
    args = parser.parse_args()
    root = args.output_parent.resolve() / ('cli-' + datetime.datetime.now().strftime('%Y%m%d-%H%M%S'))
    root.mkdir(parents=True)
    source, target = root / 'backup', root / 'cli-home'
    work, assets = root / '工作 project', root / 'assets'
    old, old_assets = r'C:\Users\Alice\Project', r'D:\Assets'
    parent, parent_path, cutoff = fixture(source, old, archived=True, label='Parent history')
    child, child_path, _ = fixture(source, old, archived=True,
        base=(parent, cutoff, parent_path.stat().st_size), label='Migrated CLI child')
    legacy, _, _ = fixture(source, old, mode='legacy', label='Legacy CLI chat')
    values = [json.loads(line) for line in child_path.read_text().splitlines()]
    values[0]['payload']['source'] = 'exec'  # ordinarily hidden by the interactive picker
    values[0]['payload']['cwd'] = r'\\?\c:\users\ALICE\project'
    values[0]['payload']['runtime_workspace_roots'] = [old, old_assets]
    child_path.write_text(''.join(json.dumps(v) + '\n' for v in values), encoding='utf-8')
    (source / 'session_index.jsonl').write_text(json.dumps({'id': child, 'thread_name': 'Migrated CLI child'}) + '\n', encoding='utf-8')
    original = logical_snapshot(source)
    cases = []

    def record(name):
        cases.append(name)
        print('PASS ' + name, flush=True)
        (args.output_parent / 'cli-runtime-results.json').write_text(json.dumps({
            'runtime': subprocess.check_output([str(args.codex_bin), '--version'], text=True).strip(),
            'root': str(root), 'cases': cases,
        }, ensure_ascii=False, indent=2), encoding='utf-8')

    def run(home, *arguments, success=True, runtime=None):
        env = dict(os.environ, CODEX_MIGRATE_CODEX_BIN=str(runtime or args.codex_bin.resolve()),
            CODEX_SQLITE_HOME=str(home), CODEX_MIGRATE_LOG_DIR=str(root / 'logs'))
        for key in ('CODEX_HOME', 'OPENAI_API_KEY', 'CODEX_API_KEY'):
            env.pop(key, None)
        result = subprocess.run([str(args.migrate_bin.resolve()), *map(str, arguments)], env=env,
            capture_output=True, text=True, encoding='utf-8', timeout=150)
        assert (result.returncode == 0) == success, result.stdout + result.stderr
        return result

    arguments = ['import', root, '--target', 'cli', '--codex-home', target,
        '--thread', child, '--thread', legacy, '--map', f'{old}={work}', '--map', f'{old_assets}={assets}']
    preview = json.loads(run(target, *arguments, '--dry-run').stdout)
    assert preview['target'] == 'cli'
    assert preview['dependency_thread_ids'] == [parent]
    assert not target.exists() and not work.exists() and not assets.exists()
    record('Windows extended paths, case-insensitive mapping and backup parent discovery; dry-run writes no target')
    result = run(target, *arguments)
    assert 'codex resume' in result.stdout
    assert work.is_dir() and assets.is_dir()
    assert logical_snapshot(source) == original
    assert not (target / '.codex-global-state.json').exists()
    record('CLI import activates selected archived/exec sessions; source backup unchanged')

    server = Server(args.codex_bin, target)
    try:
        listed = server.call('thread/list', {'cwd': str(work), 'limit': 100})['data']
        assert {t['id'] for t in listed} == {child, legacy}, listed
        assert not server.call('thread/list', {'cwd': str(assets)})['data']
        resumed = server.call('thread/resume', {'threadId': child, 'excludeTurns': True})['thread']
        assert resumed['cwd'] == str(work) and resumed['historyMode'] == 'paginated'
        items = server.call('thread/items/list', {'threadId': child, 'limit': 100})['data']
        assert len(items) == 4 and 'Parent history' in json.dumps(items) and 'Migrated CLI child' in json.dumps(items)
        legacy_history = server.call('thread/read', {'threadId': legacy, 'includeTurns': True})
        assert 'Legacy CLI chat' in json.dumps(legacy_history)
    finally:
        server.close()
    with closing(sqlite3.connect(target / 'state_5.sqlite')) as db:
        assert db.execute('SELECT archived, source, cwd FROM threads WHERE id=?', (child,)).fetchone() == (0, 'cli', str(work))
        assert db.execute('SELECT archived FROM threads WHERE id=?', (parent,)).fetchone()[0] == 1
        assert db.execute('SELECT count(*) FROM projects').fetchone()[0] == 0
    record('Native cwd-filtered interactive list and resume by UUID; full fork and legacy history readable')

    run(target, *arguments)
    before = logical_snapshot(target)
    repeated = run(target, *arguments)
    transaction = repeated.stdout.split('transaction id: ')[1].splitlines()[0]
    run(target, 'rollback', transaction, '--codex-home', target)
    assert logical_snapshot(target) == before
    record('Repeated import, duplicate detection and rollback restore complete history')

    exported = json.loads(run(target, 'export', target, '--output-parent', root / 'portable', '--thread', child, '--thread', legacy).stdout)
    backup = Path(exported['output'])
    assert exported['thread_count'] == 3
    desktop, desktop_work, desktop_assets = root / 'desktop-home', root / 'desktop-work', root / 'desktop-assets'
    run(desktop, 'import', backup, '--target', 'desktop', '--codex-home', desktop,
        '--map', f'{work}={desktop_work}', '--map', f'{assets}={desktop_assets}')
    server = Server(args.codex_bin, desktop)
    try:
        assert len(server.call('thread/items/list', {'threadId': child})['data']) == 4
        assert server.call('thread/read', {'threadId': child})['thread']['cwd'] == str(desktop_work)
    finally:
        server.close()
    record('Ubuntu export reimports through Desktop engine with project and ancestor history preserved')

    failed_target = root / 'failure-target'
    failed_work = root / 'failure-work'
    failed = run(failed_target, 'import', source, '--target', 'cli', '--codex-home', failed_target,
        '--thread', child, '--map', f'{old}={failed_work}', '--map', f'{old_assets}={assets}',
        success=False, runtime=args.migrate_bin.resolve())
    assert 'rolled back' in failed.stderr and not failed_work.exists()
    assert not list((failed_target / 'sessions').rglob('*.jsonl'))
    record('Native runtime failure rolls back imported data and newly created work folders')
    custom = root / 'custom-provider-home'
    custom.mkdir()
    (custom / 'config.toml').write_text('''model = "synthetic-test-model"
model_provider = "migration-test"
[model_providers.migration-test]
name = "Local migration test"
base_url = "http://127.0.0.1:9/v1"
wire_api = "responses"
''', encoding='utf-8')
    custom_args = ['import', source, '--target', 'cli', '--codex-home', custom,
        '--thread', child, '--map', f'{old}={work}', '--map', f'{old_assets}={assets}']
    run(custom, *custom_args)
    run(custom, *custom_args)
    server = Server(args.codex_bin, custom)
    try:
        listed = server.call('thread/list', {'cwd': str(work), 'modelProviders': ['migration-test']})['data']
        assert [t['id'] for t in listed] == [child]
    finally:
        server.close()
    record('CLI provider filter matches destination config, including fork ancestors')
    config = custom / 'config.toml'
    config.write_text(config.read_text().replace('migration-test', 'migration-test-2'), encoding='utf-8')
    run(custom, *custom_args)
    with closing(sqlite3.connect(custom / 'state_5.sqlite')) as db:
        assert db.execute('SELECT model_provider FROM threads WHERE id=?', (child,)).fetchone()[0] == 'migration-test-2'
    record('Changing the destination provider and reimporting updates existing rows without duplicate conflicts')
    custom_export = json.loads(run(custom, 'export', custom, '--output-parent', root / 'custom-export', '--thread', child).stdout)
    roundtrip = root / 'custom-roundtrip-desktop'
    run(roundtrip, 'import', custom_export['output'], '--target', 'desktop', '--codex-home', roundtrip,
        '--map', f'{work}={desktop_work}', '--map', f'{assets}={desktop_assets}')
    with closing(sqlite3.connect(roundtrip / 'state_5.sqlite')) as db:
        assert db.execute('SELECT model_provider FROM threads WHERE id=?', (child,)).fetchone()[0] == 'openai'
    record('Custom-provider Ubuntu backup imports back to Desktop with the destination provider')
    print(json.dumps({'root': str(root), 'home': str(target), 'cwd': str(work), 'thread': child}))


if __name__ == '__main__':
    main()
