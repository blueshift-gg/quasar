#!/usr/bin/env python3
import importlib.util
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "check-unsafe-policy.py"
spec = importlib.util.spec_from_file_location("check_unsafe_policy", SCRIPT)
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)


class UnsafePolicyTests(unittest.TestCase):
    def scan(self, body: str) -> list[str]:
        with tempfile.NamedTemporaryFile("w", suffix=".rs", delete=False) as handle:
            handle.write(body)
            path = Path(handle.name)
        try:
            return mod.scan_file(str(path), "demo.rs")
        finally:
            path.unlink()

    def test_unsafe_fn_without_safety_doc_is_flagged(self) -> None:
        found = self.scan("/// Reads a thing.\nunsafe fn read_it() {}\n")
        self.assertEqual(len(found), 1)
        self.assertIn("read_it", found[0])

    def test_unsafe_fn_with_safety_doc_passes(self) -> None:
        found = self.scan(
            "/// Reads a thing.\n///\n/// # Safety\n///\n"
            "/// The caller must ensure the thing exists.\n"
            "#[inline]\nunsafe fn read_it() {}\n"
        )
        self.assertEqual(found, [])

    def test_plain_comment_between_doc_and_fn_is_walked(self) -> None:
        found = self.scan(
            "/// # Safety\n/// Contract.\n// implementation note\n"
            "unsafe fn read_it() {}\n"
        )
        self.assertEqual(found, [])

    def test_bare_unsafe_block_is_flagged(self) -> None:
        found = self.scan("fn f() {\n    let x = unsafe { g() };\n}\n")
        self.assertEqual(len(found), 1)

    def test_safety_comment_above_block_passes(self) -> None:
        found = self.scan(
            "fn f() {\n    // SAFETY: g's contract holds here.\n"
            "    let x = unsafe { g() };\n}\n"
        )
        self.assertEqual(found, [])

    def test_safety_covers_its_statement_paragraph(self) -> None:
        found = self.scan(
            "fn f() {\n    // SAFETY: both reads are in bounds.\n"
            "    let a = unsafe { g() };\n    let b = unsafe { h() };\n}\n"
        )
        self.assertEqual(found, [])

    def test_blank_line_ends_safety_coverage(self) -> None:
        found = self.scan(
            "fn f() {\n    // SAFETY: covers only the first.\n"
            "    let a = unsafe { g() };\n\n    let b = unsafe { h() };\n}\n"
        )
        self.assertEqual(len(found), 1)

    def test_safety_first_inside_block_passes(self) -> None:
        found = self.scan(
            "fn f() {\n    unsafe {\n        // SAFETY: in-bounds by the const assert.\n"
            "        g();\n    }\n}\n"
        )
        self.assertEqual(found, [])

    def test_trait_impl_methods_inherit_contract(self) -> None:
        found = self.scan(
            "// SAFETY: upholds the trait contract.\n"
            "unsafe impl Alloc for Bump {\n"
            "    unsafe fn alloc(&self) -> *mut u8 { core::ptr::null_mut() }\n"
            "}\n"
        )
        self.assertEqual(found, [])

    def test_unsafe_trait_declaration_needs_safety_doc(self) -> None:
        flagged = self.scan("pub unsafe trait Marker {}\n")
        self.assertEqual(len(flagged), 1)
        passing = self.scan(
            "/// # Safety\n///\n/// Implementors must be transparent.\n"
            "pub unsafe trait Marker {}\n"
        )
        self.assertEqual(passing, [])

    def test_trailing_test_module_is_exempt(self) -> None:
        found = self.scan(
            "fn f() {}\n#[cfg(test)]\nmod tests {\n"
            "    fn t() { unsafe { g() } }\n}\n"
        )
        self.assertEqual(found, [])


if __name__ == "__main__":
    unittest.main()
