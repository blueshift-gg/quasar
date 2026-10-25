#!/usr/bin/env python3
"""Check comparison-program flags without compiling or touching real artifacts."""

import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import tempfile
import unittest


class RivalToolsTests(unittest.TestCase):
    def test_multisig_overrides_only_the_architecture(self):
        cases = [
            ("--tools-version v1.56 --arch v3", "--tools-version v1.56 --arch v0"),
            ("--tools-version v1.53 --arch v3 --verbose", "--tools-version v1.53 --verbose --arch v0"),
            ("--tools-version=v1.53 --arch=v3 --verbose", "--tools-version=v1.53 --verbose --arch v0"),
            ("--verbose --tools-version v1.53", "--verbose --tools-version v1.53 --arch v0"),
            ("--tools-version\nv1.53\t--arch\nv3 --verbose", "--tools-version v1.53 --verbose --arch v0"),
        ]
        source = Path(__file__).resolve().parents[2] / "bench/rivals.sh"
        for tools, multisig_tools in cases:
            with self.subTest(tools=tools), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                (root / "bench").mkdir()
                script = root / "bench/rivals.sh"
                shutil.copyfile(source, script)
                (root / "git").write_text("#!/bin/sh\nexit 0\n")
                (root / "cargo").write_text(
                    "#!/usr/bin/env python3\n"
                    "import json, os, sys\n"
                    "with open(os.environ['RIVALS_TEST_LOG'], 'a') as log:\n"
                    "    log.write(json.dumps(sys.argv[1:]) + '\\n')\n"
                )
                for tool in ["git", "cargo"]:
                    (root / tool).chmod(0o755)
                log = root / "calls.jsonl"
                env = dict(os.environ, PATH=f"{root}:{os.environ['PATH']}",
                           SBF_TOOLS=tools, PLATFORM_TOOLS_VERSION="v1.56",
                           RIVALS_TEST_LOG=str(log))
                subprocess.run(["sh", str(script)], env=env, check=True)
                calls = [json.loads(line) for line in log.read_text().splitlines()]
                self.assertEqual(len(calls), 7)
                for call in calls:
                    manifest_index = call.index("--manifest-path")
                    is_multisig = call[manifest_index + 1].endswith("/multisig/Cargo.toml")
                    expected = multisig_tools if is_multisig else tools
                    self.assertEqual(call[:manifest_index], ["build-sbf", *shlex.split(expected)])


if __name__ == "__main__":
    unittest.main()
