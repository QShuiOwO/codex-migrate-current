"""Check the shared release gate without uploading anything."""
import hashlib
from pathlib import Path
import runpy
import tempfile
import unittest

verify = runpy.run_path(str(Path(__file__).resolve().parents[1] / 'scripts/check-release-assets.py'))['verify']


class ReleaseAssetsTest(unittest.TestCase):
    def test_requires_both_current_archives_and_valid_checksums(self):
        for damage in (None, 'missing_platform', 'wrong_hash', 'old_version', 'extra_file'):
            with self.subTest(damage=damage), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                for platform in ('Windows-x64.zip', 'Ubuntu-24.04-x64.tar.gz'):
                    name = f'Codex-Migrate-Current-1.1.0-current.7-{platform}'
                    (root / name).write_bytes(b'synthetic package')
                    (root / (name + '.sha256')).write_text(hashlib.sha256(b'synthetic package').hexdigest() + '  ' + name + '\n')
                if damage == 'missing_platform':
                    next(root.glob('*.tar.gz')).unlink()
                elif damage == 'wrong_hash':
                    next(root.glob('*.zip')).write_bytes(b'changed')
                elif damage == 'extra_file':
                    (root / 'private-data.json').write_text('{}')
                if damage is None:
                    verify(root, '1.1.0-current.7')
                else:
                    with self.assertRaises(ValueError):
                        verify(root, '1.1.0-current.6' if damage == 'old_version' else '1.1.0-current.7')


if __name__ == '__main__':
    unittest.main()
