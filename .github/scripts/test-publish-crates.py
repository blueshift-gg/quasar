"""Check release ordering and retries with mocked network and upload commands."""

import os
from pathlib import Path
import subprocess
import tempfile

SCRIPT = Path(__file__).with_name("publish-crates.sh").resolve()
PACKAGES = ['quasar-derive', 'quasar-lang', 'quasar-spl', 'quasar-metadata', 'quasar-test', 'quasar-schema', 'quasar-idl-schema', 'quasar-idl', 'quasar-profile', 'quasar-cli']

with tempfile.TemporaryDirectory() as directory:
    root = Path(directory)
    log = root / "uploads"
    (root / "curl").write_text(r"""#!/usr/bin/env python3
import os, sys
package = sys.argv[-1].split('/')[-2]
if os.environ.get('NETWORK_ERROR'):
    sys.exit(7)
print('200' if package in os.environ.get('EXISTING', '').split(',')
      else os.environ.get('HTTP_STATUS', '404'), end='')
""")
    (root / "cargo").write_text(r"""#!/usr/bin/env python3
import os, sys
assert sys.argv[1:6] == ['publish', '--locked', '--registry', 'crates-io', '--package']
with open(os.environ['UPLOAD_LOG'], 'a') as log:
    log.write(sys.argv[6] + '\n')
sys.exit(1 if sys.argv[6] == os.environ.get('FAIL_PACKAGE') else 0)
""")
    for command in ["curl", "cargo"]:
        (root / command).chmod(0o755)

    cases = [
        ({}, PACKAGES, True),
        ({"EXISTING": ",".join(PACKAGES[:2])}, PACKAGES[2:], True),
        ({"EXISTING": ",".join(PACKAGES)}, [], True),
        ({"HTTP_STATUS": "503"}, [], False),
        ({"NETWORK_ERROR": "1"}, [], False),
        ({"FAIL_PACKAGE": "quasar-lang"}, PACKAGES[:2], False),
    ]
    for settings, expected, succeeds in cases:
        log.write_text("")
        env = {
            **os.environ,
            "PATH": str(root) + os.pathsep + os.environ["PATH"],
            "RELEASE_TAG": "v0.2.0",
            "UPLOAD_LOG": str(log),
            **settings,
        }
        result = subprocess.run(["sh", str(SCRIPT)], env=env, capture_output=True, text=True)
        assert (result.returncode == 0) == succeeds, (settings, result.stderr)
        assert log.read_text().splitlines() == expected, (settings, log.read_text())

print("Release ordering, partial retries and failure checks passed (no uploads).")
