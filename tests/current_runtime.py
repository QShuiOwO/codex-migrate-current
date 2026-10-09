"""Opt-in integration tests against a real Codex runtime, with synthetic data only.

python tests/current_runtime.py --codex-bin ABSOLUTE_CODEX_EXE --migrate-bin ABSOLUTE_MIGRATE_EXE
No real .codex data, credentials, model calls, or turn/start requests are used.
"""
from __future__ import annotations
import argparse
import datetime
from contextlib import closing
import json
import os
from pathlib import Path
import queue
import re
import shutil
import sqlite3
import subprocess
import threading
import time
import uuid


class Server:
    def __init__(self, executable, home, sqlite_home=None):
        env = dict(os.environ, CODEX_HOME=str(home), CODEX_SQLITE_HOME=str(sqlite_home or home))
        for key in ('OPENAI_API_KEY', 'CODEX_API_KEY'):
            env.pop(key, None)
        self.process = subprocess.Popen([str(executable), 'app-server', '--listen', 'stdio://'], env=env,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            text=True, encoding='utf-8', creationflags=0x08000000 if os.name == 'nt' else 0)
        self.messages = queue.Queue()
        self.identifier = 0
        def reader():
            for line in self.process.stdout:
                try: self.messages.put(json.loads(line))
                except json.JSONDecodeError: pass
        threading.Thread(target=reader, daemon=True).start()
        self.call('initialize', {'clientInfo': {'name': 'codex_migrate_test', 'version': '1.1.0'}, 'capabilities': {'experimentalApi': True}})
        self.send({'method': 'initialized', 'params': {}})

    def send(self, value):
        self.process.stdin.write(json.dumps(value) + '\n')
        self.process.stdin.flush()

    def call(self, method, params):
        self.identifier += 1
        self.send({'id': self.identifier, 'method': method, 'params': params})
        deadline = time.monotonic() + 45
        while True:
            value = self.messages.get(timeout=max(0, deadline - time.monotonic()))
            if value.get('id') == self.identifier:
                if 'error' in value: raise RuntimeError(f'{method}: {value["error"]}')
                return value['result']

    def close(self):
        self.process.kill()
        self.process.wait(timeout=10)
        self.process.stdin.close()
        self.process.stdout.close()


def fixture(home, cwd, mode='paginated', base=None, item_count=2, archived=False, identifier=None, label='Synthetic'):
    identifier, turn = identifier or str(uuid.uuid4()), str(uuid.uuid4())
    timestamp = '2026-10-01T00:00:00Z'
    records = [
        {'type': 'session_meta', 'payload': {'id': identifier, 'timestamp': timestamp, 'cwd': str(cwd),
            'source': 'vscode', 'originator': 'Codex Desktop', 'thread_source': 'user', 'model_provider': 'openai', 'cli_version': '0.162.0-alpha.2',
            'history_mode': mode, 'runtime_workspace_roots': [str(cwd)]}},
        {'type': 'event_msg', 'payload': {'type': 'task_started', 'turn_id': turn, 'root_turn_id': turn,
            'model_context_window': 272000, 'collaboration_mode_kind': 'default'}},
    ]
    for index in range(item_count):
        user = index == 0
        text = f'{label} {index}'
        item = {'type': 'UserMessage' if user else 'AgentMessage', 'id': str(uuid.uuid4()),
            'content': [{'type': 'text' if user else 'Text', 'text': text}]}
        if user:
            item['content'][0]['text_elements'] = []
            item['client_id'] = None
        else: item['phase'] = 'final_answer'
        if mode == 'paginated':
            records.append({'type': 'event_msg', 'payload': {'type': 'item_completed', 'thread_id': identifier,
                'turn_id': turn, 'started_at_ms': 1790812800000, 'completed_at_ms': 1790812800100, 'item': item}})
        else:
            records += [{'type': 'event_msg', 'payload': {'type': 'user_message' if user else 'agent_message',
                'message': text, 'images': [], 'local_images': [], 'text_elements': []}},
                {'type': 'response_item', 'payload': {'type': 'message', 'role': 'user' if user else 'assistant',
                    'content': [{'type': 'input_text' if user else 'output_text', 'text': text}]}}]
    records.append({'type': 'event_msg', 'payload': {'type': 'task_complete', 'turn_id': turn, 'last_agent_message': 'Synthetic reply'}})
    start = 0
    if base:
        parent_id, cutoff, byte_offset = base
        records[0]['payload'].update({'forked_from_id': parent_id, 'forked_from_ordinal_exclusive': cutoff,
            'history_base': {'thread_id': parent_id, 'end_ordinal_exclusive': cutoff, 'end_byte_offset': byte_offset}})
        start = cutoff
    for ordinal, record in enumerate(records, start):
        record.update(timestamp=timestamp, ordinal=ordinal)
    path = home / ('archived_sessions' if archived else 'sessions/2026/10/01') / f'rollout-2026-10-01T00-00-00-{identifier}.jsonl'
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(''.join(json.dumps(v, separators=(',', ':')) + '\n' for v in records), encoding='utf-8', newline='\n')
    return identifier, path, len(records)


def logical_snapshot(home):
    result = {}
    for name in ('state_5.sqlite', 'thread_history_1.sqlite'):
        path = home / name
        if not path.exists(): continue
        with closing(sqlite3.connect(path)) as db:
            result[name] = {table: sorted(db.execute(f'SELECT * FROM "{table}"').fetchall(), key=repr)
                for (table,) in db.execute("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name!='_sqlx_migrations'").fetchall()}
    result['rollouts'] = {str(p.relative_to(home)): p.read_bytes().hex() for folder in ('sessions', 'archived_sessions') for p in (home / folder).rglob('*.jsonl')}
    for name in ('session_index.jsonl', '.codex-global-state.json'):
        if (home / name).exists(): result[name] = (home / name).read_bytes().hex()
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--codex-bin', type=Path, required=True)
    parser.add_argument('--migrate-bin', type=Path, required=True)
    parser.add_argument('--output-parent', type=Path, default=Path(__file__).resolve().parents[1] / 'test-results')
    args = parser.parse_args()
    root = args.output_parent.resolve() / ('runtime-' + datetime.datetime.now().strftime('%Y%m%d-%H%M%S'))
    root.mkdir(parents=True)
    results = {'runtime': subprocess.check_output([str(args.codex_bin), '--version'], text=True).strip(), 'cases': [], 'root': str(root)}

    def run(home, arguments, success=True, sqlite_home=None, runtime=True, executable_override=None):
        env = dict(os.environ, CODEX_MIGRATE_CODEX_BIN=str((executable_override or args.codex_bin).resolve()) if runtime else '', CODEX_SQLITE_HOME=str(sqlite_home or home), CODEX_MIGRATE_LOG_DIR=str(root / 'diagnostic-logs'))
        env.pop('CODEX_HOME', None)
        for key in ('OPENAI_API_KEY', 'CODEX_API_KEY'): env.pop(key, None)
        completed = subprocess.run([str(args.migrate_bin.resolve()), *map(str, arguments)], env=env,
            capture_output=True, text=True, encoding='utf-8', timeout=150)
        if (completed.returncode == 0) != success:
            raise AssertionError(f'{arguments}: exit={completed.returncode}\n{completed.stdout}\n{completed.stderr}')
        return completed

    def history(home, identifier, sqlite_home=None):
        server = Server(args.codex_bin, home, sqlite_home)
        try:
            turns = server.call('thread/turns/list', {'threadId': identifier, 'itemsView': 'full', 'limit': 100})
            cursor, count = None, 0
            while True:
                params = {'threadId': identifier, 'limit': 100}
                if cursor: params['cursor'] = cursor
                page = server.call('thread/items/list', params)
                count += len(page['data'])
                cursor = page.get('nextCursor')
                if not cursor: break
            return len(turns['data']), count
        finally: server.close()

    def record(name, **data):
        results['cases'].append({'name': name, 'passed': True, **data})
        print(json.dumps(results['cases'][-1], ensure_ascii=False), flush=True)
        (args.output_parent / 'current-runtime-results.json').write_text(json.dumps(results, ensure_ascii=False, indent=2), encoding='utf-8')

    source, target = root / 'source', root / 'target'
    old, old_second, new, new_second = [root / p for p in ('old-work', 'old-second', 'new-work', 'new-second')]
    for path in (old, old_second, new, new_second): path.mkdir()
    parent, parent_path, cutoff = fixture(source, old, label='Parent synthetic')
    child, child_path, _ = fixture(source, old, base=(parent, cutoff, parent_path.stat().st_size), label='Child synthetic')
    values = [json.loads(line) for line in child_path.read_text().splitlines()]
    values[0]['payload']['runtime_workspace_roots'].append(str(old_second))
    child_path.write_text(''.join(json.dumps(v) + '\n' for v in values), encoding='utf-8')
    server = Server(args.codex_bin, source)
    try:
        for identifier, path in ((parent, parent_path), (child, child_path)):
            server.call('thread/resume', {'threadId': identifier, 'path': str(path), 'cwd': str(old), 'excludeTurns': True})
        server.call('thread/name/set', {'threadId': child, 'name': '分页分叉测试'})
        server.call('project/import', {'idempotencyKey': 'synthetic-source-project', 'name': '多根项目测试',
            'roots': [{'path': str(old)}, {'path': str(old_second)}], 'metadata': {'synthetic': 'true'}, 'threads': [parent, child]})
        server.call('thread/archive', {'threadId': parent})
    finally: server.close()
    mappings = ['--map', f'{old}={new}', '--map', f'{old_second}={new_second}']
    arguments = ['import', source, '--codex-home', target, '--thread', child, *mappings]
    dry = run(target, [*arguments, '--dry-run'])
    plan = json.loads(dry.stdout)
    assert plan['dependency_thread_ids'] == [parent]
    assert [t['thread']['id'] for t in plan['threads']] == [parent, child]
    assert not target.exists()
    record('dry-run and ancestor closure')
    source_before = logical_snapshot(source)
    completed = run(target, arguments)
    assert logical_snapshot(source) == source_before
    assert history(target, child) == (2, 4)
    with closing(sqlite3.connect(target / 'state_5.sqlite')) as db:
        assert db.execute('SELECT count(*) FROM projects').fetchone()[0] == 1
        assert db.execute('SELECT name FROM threads WHERE id=?', (child,)).fetchone()[0] == '分页分叉测试'
        roots = [r[0] for r in db.execute('SELECT path FROM project_roots ORDER BY position')]
        assert roots == [str(new), str(new_second)], roots
        assert db.execute('SELECT count(*) FROM threads WHERE history_mode="paginated"').fetchone()[0] == 2
        assert db.execute('SELECT archived FROM threads WHERE id=?', (parent,)).fetchone()[0] == 1
    record('fresh target, paginated fork, native multiroot project, source unchanged')
    html_root = root / 'html'
    run(target, ['export-html', '--codex-home', target, '--thread', child, '--output', html_root])
    html = next(html_root.glob('*.html')).read_text(encoding='utf-8')
    assert 'Parent synthetic 0' in html and 'Child synthetic 0' in html
    record('paginated fork HTML includes ancestor messages')
    run(target, arguments)
    alias = root / 'renamed-backup'
    shutil.copytree(source, alias)
    run(target, ['import', alias, '--codex-home', target, '--thread', child, *mappings])
    assert history(target, child) == (2, 4)
    with closing(sqlite3.connect(target / 'state_5.sqlite')) as db:
        assert db.execute('SELECT count(*) FROM projects').fetchone()[0] == 1
    record('repeat import without duplicate histories/projects')
    before_repeat = logical_snapshot(target)

    rebind_work, rebind_second = root / 'rebound-work', root / 'rebound-second'
    rebind_work.mkdir(); rebind_second.mkdir()
    rebound = run(target, ['rebind', '--codex-home', target, '--thread', child,
        '--map', f'{new}={rebind_work}', '--map', f'{new_second}={rebind_second}'])
    assert history(target, child) == (2, 4)
    with closing(sqlite3.connect(target / 'state_5.sqlite')) as db:
        assert all(Path(cwd) == rebind_work for (cwd,) in db.execute('SELECT cwd FROM threads'))
    transaction = re.search(r'"transaction_id"\s*:\s*"([^"]+)"', rebound.stdout).group(1)
    run(target, ['rollback', transaction, '--codex-home', target])
    assert logical_snapshot(target) == before_repeat, 'manual rollback failed to restore display history/state/rollouts'
    record('rebind cached paginated fork and full manual rollback')

    many_source, many_target = root / 'many-source', root / 'many-target'
    many, _, _ = fixture(many_source, old, item_count=205)
    run(many_target, ['import', many_source, '--codex-home', many_target, '--thread', many, *mappings])
    run(many_target, ['import', many_source, '--codex-home', many_target, '--thread', many, *mappings])
    assert history(many_target, many) == (1, 205)
    with closing(sqlite3.connect(many_target / 'state_5.sqlite')) as db:
        assert db.execute('SELECT count(*) FROM projects').fetchone()[0] == 1
    record('history item pagination beyond first 100 items', items=205)

    broken = root / 'broken-source'
    missing, _, _ = fixture(broken, old, base=(str(uuid.uuid4()), 5, 100))
    before = logical_snapshot(target)
    rejected = run(target, ['import', broken, '--codex-home', target, '--thread', missing, *mappings], success=False)
    assert 'missing history_base ancestor' in rejected.stderr
    assert logical_snapshot(target) == before
    record('missing ancestor rejected before mutation')

    # Match a backup moved to another machine: its additional root is absent locally.
    extra_source, extra_target = root / 'extra-source', root / 'extra-target'
    extra_id, extra_path, _ = fixture(extra_source, old)
    extra_records = [json.loads(line) for line in extra_path.read_text().splitlines()]
    extra_records[0]['payload']['runtime_workspace_roots'].append(str(root / 'missing-extra-root'))
    extra_path.write_text(''.join(json.dumps(v) + '\n' for v in extra_records), encoding='utf-8')
    rejected = run(extra_target, ['import', extra_source, '--codex-home', extra_target, '--thread', extra_id, *mappings, '--dry-run'], success=False)
    assert 'workspace root requires an explicit mapping' in rejected.stderr
    assert not extra_target.exists()
    failed_logs = [p.read_text(encoding='utf-8') for p in (root / 'diagnostic-logs').glob('*.log')]
    assert any(extra_id in value and 'missing-extra-root' in value and 'RESULT failure' in value for value in failed_logs)
    record('missing extra root rejected during preview with persistent diagnostic log')


    bad_source = root / 'bad-source'
    valid, _, _ = fixture(bad_source, old, identifier='00000000-0000-4000-8000-000000000001')
    bad, bad_path, _ = fixture(bad_source, old, identifier='ffffffff-ffff-4fff-8fff-ffffffffffff')
    records = [json.loads(line) for line in bad_path.read_text().splitlines()]
    records[2]['payload']['item']['type'] = 'UnsupportedFutureItem'
    bad_path.write_text(''.join(json.dumps(v) + '\n' for v in records), encoding='utf-8')
    before = logical_snapshot(target)
    rejected = run(target, ['import', bad_source, '--codex-home', target, '--thread', valid, '--thread', bad, *mappings], success=False)
    assert 'rolled back' in rejected.stderr, rejected.stderr
    assert logical_snapshot(target) == before
    record('native registration failure rolls back all DBs and rollouts')

    failed_logs = [p.read_text(encoding='utf-8') for p in (root / 'diagnostic-logs').glob('*.log')]
    assert any(bad in value and 'RESULT failure' in value and 'Runtime=' in value and 'rolled back' in value and 'lost 1 completed item' in value for value in failed_logs)
    record('projection failure log retains runtime, root cause and rollback result')

    before = logical_snapshot(target)
    rejected = run(target, ['import', many_source, '--codex-home', target, '--thread', many, *mappings], success=False, executable_override=args.migrate_bin)
    assert 'Native RPC initialize' in rejected.stderr and 'stderr tail:' in rejected.stderr
    assert 'app-server' in rejected.stderr and 'rolled back' in rejected.stderr
    assert logical_snapshot(target) == before
    failed_logs = [p.read_text(encoding='utf-8') for p in (root / 'diagnostic-logs').glob('*.log')]
    assert any('Native RPC initialize' in value and 'app-server' in value and 'RESULT failure' in value for value in failed_logs)
    record('invalid runtime records startup stderr and restores target')

    legacy_source, legacy_target = root / 'legacy-source', root / 'legacy-target'
    legacy, _, _ = fixture(legacy_source, old, mode='legacy')
    run(legacy_target, ['import', legacy_source, '--codex-home', legacy_target, '--thread', legacy, *mappings])
    with closing(sqlite3.connect(legacy_target / 'state_5.sqlite')) as db:
        assert db.execute('SELECT history_mode FROM threads WHERE id=?', (legacy,)).fetchone()[0] == 'legacy'
    record('legacy history preserved under current runtime')

    missing_runtime_target = root / 'no-runtime'
    rejected = run(missing_runtime_target, ['import', many_source, '--codex-home', missing_runtime_target, '--thread', many, *mappings], success=False, runtime=False)
    assert 'native Codex runtime is required' in rejected.stderr
    assert not missing_runtime_target.exists()
    record('paginated migration without runtime rejected before writes')

    archived_source, archived_target = root / 'archived-source', root / 'archived-target'
    archived, _, _ = fixture(archived_source, old, archived=True)
    run(archived_target, ['import', archived_source, '--codex-home', archived_target, '--thread', archived, *mappings])
    with closing(sqlite3.connect(archived_target / 'state_5.sqlite')) as db:
        archived_flag, archived_path = db.execute('SELECT archived,rollout_path FROM threads WHERE id=?', (archived,)).fetchone()
        assert archived_flag == 1 and Path(archived_path).is_file() and 'archived_sessions' in archived_path
    assert history(archived_target, archived) == (1, 2)
    record('archived paginated thread stays archived and readable')

    stub_target = root / 'history-only'
    run(stub_target, ['import', source, '--codex-home', stub_target, '--thread', child, '--history-only', old])
    assert history(stub_target, child) == (2, 4)
    with closing(sqlite3.connect(stub_target / 'state_5.sqlite')) as db:
        assert db.execute('SELECT count(*) FROM projects').fetchone()[0] == 0
        assert db.execute('SELECT count(*) FROM threads WHERE project_id IS NOT NULL').fetchone()[0] == 0
    record('history-only fork uses stub workspace without native project')

    split_target, split_db = root / 'split-home', root / 'split-db'
    completed = run(split_target, ['import', many_source, '--codex-home', split_target, '--thread', many, *mappings], sqlite_home=split_db)
    assert not (split_target / 'state_5.sqlite').exists() and (split_db / 'state_5.sqlite').exists()
    assert history(split_target, many, split_db) == (1, 205)
    transaction = completed.stdout.rsplit('transaction id: ', 1)[1].strip()
    run(split_target, ['rollback', transaction, '--codex-home', split_target], sqlite_home=split_db)
    assert not (split_db / 'state_5.sqlite').exists() and not (split_db / 'thread_history_1.sqlite').exists()
    record('separate CODEX_SQLITE_HOME and fresh-target rollback')
    print(json.dumps({'passed': len(results['cases']), 'results': str(args.output_parent / 'current-runtime-results.json')}, ensure_ascii=False))


if __name__ == '__main__': main()
