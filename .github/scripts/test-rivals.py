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
    def test_checkout_fetches_only_missing_revisions(self):
        source = Path(__file__).resolve().parents[2] / "bench/rivals.sh"
        checkout = source.read_text().split("checkout() {", 1)[1].split("\n}", 1)[0]
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "src/anchor-next").mkdir(parents=True)
            git = root / "git"
            git.write_text(
                "#!/bin/sh\n"
                'printf "%s\\n" "$*" >> "$GIT_TEST_LOG"\n'
                '[ "$3" != cat-file ] || [ "$REVISION_CACHED" = 1 ]\n'
            )
            git.chmod(0o755)
            log = root / "git.log"
            for cached in ["0", "1"]:
                with self.subTest(cached=cached):
                    log.write_text("")
                    env = dict(os.environ, PATH=f"{root}:{os.environ['PATH']}",
                               GIT_TEST_LOG=str(log), REVISION_CACHED=cached)
                    subprocess.run(
                        ["sh", "-ec", 'out="$1"; checkout() {' + checkout
                         + '\n}; checkout anchor-next https://example.com/anchor revision',
                         "test", str(root)], env=env, check=True,
                    )
                    calls = log.read_text().splitlines()
                    self.assertEqual(any(" fetch " in call for call in calls), cached == "0")
                    self.assertTrue(calls[-1].endswith("checkout --quiet revision"))

    def test_multisig_overrides_only_the_architecture(self):
        cases = [
            ("--tools-version v1.57 --arch v3", "--tools-version v1.57 --arch v0"),
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
                           SBF_TOOLS=tools, PLATFORM_TOOLS_VERSION="v1.57",
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
