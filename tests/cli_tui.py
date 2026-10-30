"""Linux PTY smoke test of the actual resume picker using cli_runtime.py fixtures.

Only opens synthetic history with a loopback provider; sends no chat message.
"""
import argparse
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import signal
import struct
import termios
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--codex-bin', type=Path, required=True)
    parser.add_argument('--results', type=Path, default=Path('test-results/cli-runtime-results.json'))
    args = parser.parse_args()
    root = Path(json.loads(args.results.read_text())['root'])
    home, work = root / 'custom-provider-home', root / '工作 project'
    assert 'http://127.0.0.1:9/v1' in (home / 'config.toml').read_text()
    env = dict(os.environ, CODEX_HOME=str(home), CODEX_SQLITE_HOME=str(home), TERM='xterm-256color')
    for key in ('OPENAI_API_KEY', 'CODEX_API_KEY'):
        env.pop(key, None)
    pid, fd = pty.fork()
    if pid == 0:
        os.chdir(work)
        os.execvpe(str(args.codex_bin.resolve()), [str(args.codex_bin.resolve()), 'resume', '--no-daemon', '--no-alt-screen'], env)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack('HHHH', 32, 120, 0, 0))
    output = bytearray()
    selected = trusted = False
    deadline = time.monotonic() + 30
    try:
        while time.monotonic() < deadline:
            if not select.select([fd], [], [], 0.2)[0]:
                continue
            try:
                chunk = os.read(fd, 65536)
            except OSError:
                break
            if not chunk:
                break
            output.extend(chunk)
            if b'\x1b[6n' in chunk:
                os.write(fd, b'\x1b[1;1R')
            text = re.sub(r'\x1b\[[0-?]*[ -/]*[@-~]', '', output.decode('utf-8', errors='replace'))
            if not selected and 'Migrated CLI child' in text:
                assert 'Resume a previous session' in text
                os.write(fd, b'\r')
                selected = True
            if not trusted and 'Trust and continue' in text:
                # The trust dialog deliberately ignores keys immediately after
                # opening, to prevent accepting it with a buffered Enter.
                time.sleep(1)
                os.write(fd, b'\r')
                trusted = True
            if selected and 'Parent history 0' in text and 'Migrated CLI child 1' in text:
                assert 'Ask Codex to do anything' in text
                (root / 'resume-tui.txt').write_text(text, encoding='utf-8')
                print('PASS actual codex resume picker -> Enter -> fork history and input prompt')
                return
        raise AssertionError('Resume TUI did not show restored history; see resume-tui.raw')
    finally:
        (root / 'resume-tui.raw').write_bytes(output)
        os.kill(pid, signal.SIGTERM)
        os.waitpid(pid, 0)
        os.close(fd)


if __name__ == '__main__':
    main()
