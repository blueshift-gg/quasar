"""Regression tests for repository policy enforcement, without editing the repo."""

import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("policies", Path(__file__).with_name("check-policies.py"))
policies = importlib.util.module_from_spec(spec)
spec.loader.exec_module(policies)


class PolicyTests(unittest.TestCase):
    def test_lint_inheritance_is_parsed_as_toml(self):
        self.assertEqual(policies.lint_opt_in({"crate/Cargo.toml": 'lints.workspace = true\n'}), [])
        self.assertEqual(policies.lint_opt_in({"crate/Cargo.toml": '[lints]\nworkspace = true # inherited\n'}), [])
        for text in ('', '# [lints]\n# workspace = true', '[lints]\nworkspace = false', '[package.metadata]\nworkspace = true'):
            with self.subTest(text=text):
                self.assertEqual(len(policies.lint_opt_in({"new/Cargo.toml": text})), 1)

    def test_workspace_members_include_new_crates_but_not_dependencies(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'member.toml').write_text('')
            (root / 'dependency.toml').write_text('')
            metadata = {'workspace_members': ['member'], 'packages': [
                {'id': name, 'manifest_path': str(root / f'{name}.toml')}
                for name in ('member', 'dependency')
            ]}
            with patch.object(policies.subprocess, 'check_output', return_value=policies.json.dumps(metadata)):
                errors = policies.check(root, 'workspace-lints')
            self.assertEqual(len(errors), 1)
            self.assertIn('member.toml', errors[0])

    def test_metadata_failure_is_not_a_pass(self):
        with patch.object(policies.subprocess, 'check_output', side_effect=subprocess.CalledProcessError(1, 'cargo')):
            with self.assertRaises(subprocess.CalledProcessError):
                policies.check(Path('.'), 'workspace-lints')

    def test_panic_macros_are_caught_across_paths_and_delimiters(self):
        for text in ('panic!("bad")', 'core::panic! { "bad" }', 'unreachable !\n()', 'todo![]', 'unimplemented!()', 'panic /* why */ ! ("bad")'):
            with self.subTest(text=text):
                errors = policies.runtime_panics({'metadata/src/new.rs': text}, {})
                self.assertEqual(len(errors), 1)
                self.assertIn('metadata/src/new.rs:1:', errors[0])

    def test_comments_and_literals_are_not_executable_code(self):
        source = '''// panic!("comment"); process::exit(1);
/* nested /* todo!() */ sleep(1); */
let a = "panic!(\\\"quoted\\\"); exit(1); split_whitespace()";
let b = r###"unimplemented!(); "sleep""###;
let c = br##"unreachable!(); exit(2)"##;
let d = '\\"'; let e = b'\\''; let f = '\\u{1f980}';
fn borrowed<'a>(s: &'a str) {}
'''
        self.assertEqual(policies.runtime_panics({'lang/src/new.rs': source}, {}), [])
        self.assertEqual(policies.cli_conventions({'cli/src/new.rs': source}), [])
        self.assertEqual(policies.mask_non_code(source).count('\n'), source.count('\n'))

    def test_exception_is_limited_to_call_path_and_count(self):
        path, call = 'lang/src/host.rs', 'panic!("host abort")'
        exceptions = {(path, call): (1, 'Host-only program abort.')}
        self.assertEqual(policies.runtime_panics({path: call}, exceptions), [])
        self.assertTrue(policies.runtime_panics({path: call + ';' + call}, exceptions))
        self.assertTrue(policies.runtime_panics({'lang/src/other.rs': call}, exceptions))
        self.assertTrue(policies.runtime_panics({path: 'panic!("new failure")'}, exceptions))
        self.assertTrue(policies.runtime_panics({}, exceptions), 'removed calls must retire their exceptions')
        self.assertTrue(policies.runtime_panics({path: call}, {(path, call): (1, '')}))

    def test_macro_delimiters_inside_strings_are_ignored(self):
        call = 'panic!(format!("{{ ) }}", value))'
        self.assertEqual(list(policies.panic_calls(call)), [(1, call)])

    def test_unterminated_source_fails_closed(self):
        for source in ('/* unfinished', 'r##"unfinished', '"unfinished', 'panic!('):
            with self.subTest(source=source), self.assertRaises(ValueError):
                list(policies.panic_calls(source))

    def test_cli_rules_reject_calls_and_renamed_imports(self):
        for source in ('std::process::exit(1)', 'process :: exit (1)', 'use std::process::{exit as quit}; quit(1);',
                       'std::thread::sleep(delay)', 'use std::thread::sleep as wait; wait(delay);',
                       'command.split_whitespace()', 'command.split_ascii_whitespace()'):
            with self.subTest(source=source):
                self.assertTrue(policies.cli_conventions({'cli/src/new.rs': source}))

    def test_cli_boundaries_and_quote_aware_parsing_are_allowed(self):
        self.assertEqual(policies.cli_conventions({
            'cli/src/main.rs': 'std::process::exit(1);',
            'cli/src/build_watch.rs': 'std::thread::sleep(delay);',
            'cli/src/config.rs': 'shlex::split(command); Command::new(program).args(arguments);',
        }), [])
        self.assertTrue(policies.cli_conventions({'cli/src/main.rs': 'sleep(delay);'}))
        self.assertTrue(policies.cli_conventions({'cli/src/build_watch.rs': 'process::exit(1);'}))


if __name__ == '__main__':
    unittest.main()
