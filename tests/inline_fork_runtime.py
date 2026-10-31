"""Opt-in regression: inline ancestor headers must not collapse fork IDs.

Synthetic data only; no personal history or model requests.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

from current_runtime import Server, fixture


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--codex-bin', type=Path, required=True)
    parser.add_argument('--migrate-bin', type=Path, required=True)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='codex-inline-fork-') as temporary:
        root = Path(temporary)
        source, target = root / 'source', root / 'target'
        old, new = root / 'old-project', root / 'new-project'
        old.mkdir()
        parent, path, _ = fixture(source, old, label='Parent inline history')
        inherited = [json.loads(line) for line in path.read_text().splitlines()]
        children = []
        originals = {}
        for label in ('First fork', 'Second fork'):
            child, path, _ = fixture(source, old, label=label)
            rows = [json.loads(line) for line in path.read_text().splitlines()]
            rows[0]['payload']['forked_from_id'] = parent
            rows[1:1] = json.loads(json.dumps(inherited))
            for ordinal, row in enumerate(rows):
                row['ordinal'] = ordinal
            path.write_text(''.join(json.dumps(row) + '\n' for row in rows), encoding='utf-8')
            children.append(child)
            originals[path] = path.read_bytes()
        env = dict(os.environ, CODEX_SQLITE_HOME=str(target),
            CODEX_MIGRATE_CODEX_BIN=str(args.codex_bin.resolve()), CODEX_MIGRATE_LOG_DIR=str(root / 'logs'))
        for key in ('CODEX_HOME', 'OPENAI_API_KEY', 'CODEX_API_KEY'):
            env.pop(key, None)

        def run(*arguments):
            result = subprocess.run([str(args.migrate_bin.resolve()), *map(str, arguments)],
                env=env, capture_output=True, text=True, encoding='utf-8', timeout=120)
            assert result.returncode == 0, result.stdout + result.stderr
            return result.stdout

        summary = json.loads(run('export', source, '--output-parent', root / 'export',
            '--thread', children[0], '--thread', children[1]))
        assert summary['thread_count'] == 2 and summary['dependency_count'] == 0
        backup = Path(summary['output'])
        assert all(path.read_bytes() == content for path, content in originals.items())
        run('import', backup, '--codex-home', target, '--target', 'cli', '--map', f'{old}={new}')
        server = Server(args.codex_bin, target)
        try:
            visible = server.call('thread/list', {'cwd': str(new)})['data']
            assert {thread['id'] for thread in visible} == set(children)
            for child, label in zip(children, ('First fork', 'Second fork')):
                resumed = server.call('thread/resume', {'threadId': child, 'excludeTurns': True})
                assert resumed['thread']['id'] == child
                items = server.call('thread/items/list', {'threadId': child})['data']
                assert len(items) == 4, items
                text = json.dumps(items)
                assert 'Parent inline history' in text and label in text
                other = 'Second fork' if label == 'First fork' else 'First fork'
                assert other not in text
        finally:
            server.close()
        print('PASS inline forks: separate IDs, selected export, source unchanged, native import/resume and four history items per child')


if __name__ == '__main__':
    main()
