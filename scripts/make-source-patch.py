"""Generate a reviewable source patch against the original sibling checkout."""
import argparse
import difflib
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('--original', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
allowed = {'.rs', '.toml', '.md', '.ps1', '.py', '.json'}
excluded = {'.git', 'target', 'dist', 'test-results', '__pycache__'}
patch, changed = [], []
for path in sorted(project.rglob('*')):
    relative = path.relative_to(project)
    if not path.is_file() or any(part in excluded for part in relative.parts): continue
    if path.suffix not in allowed and path.name not in {'.gitignore', 'Cargo.lock'}: continue
    original = args.original / relative
    after = path.read_text(encoding='utf-8-sig')
    before = original.read_text(encoding='utf-8-sig') if original.is_file() else ''
    if before == after: continue
    changed.append(str(relative).replace('\\', '/'))
    for line in difflib.unified_diff(before.splitlines(keepends=True), after.splitlines(keepends=True),
        fromfile='a/' + relative.as_posix() if original.exists() else '/dev/null', tofile='b/' + relative.as_posix()):
        patch.append(line if line.endswith('\n') else line + '\n\\ No newline at end of file\n')
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(''.join(patch), encoding='utf-8', newline='\n')
print(f'{len(changed)} changed source/document files; patch: {args.output}')
