#!/usr/bin/env python3

import importlib.util
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parent.parent / "host-test-inventory.py"
SPEC = importlib.util.spec_from_file_location("host_test_inventory", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
INVENTORY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(INVENTORY)


class HostTestInventoryTests(unittest.TestCase):
    def test_raw_template_tests_are_not_counted_as_rust_attributes(self):
        source = '''
#[test]
fn real_test() {}

const TEMPLATE: &str = r##"
#[test]
fn generated_test() {}
"##;
'''
        stripped = INVENTORY.without_raw_strings(source)
        self.assertEqual(len(INVENTORY.TEST_ATTRIBUTE.findall(stripped)), 1)

    def test_specialized_targets_keep_their_required_runners(self):
        self.assertEqual(
            INVENTORY.runners_for("quasar-cli", "generated_clients_smoke"),
            ["make generated-client-smoke"],
        )
        self.assertEqual(
            INVENTORY.runners_for("quasar-lang", "miri"),
            ["make test-host", "make test-miri"],
        )


if __name__ == "__main__":
    unittest.main()
