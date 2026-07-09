#![no_main]

use libfuzzer_sys::fuzz_target;

// Fuzz the compact `#[zeropod(compact)]` decode path (validate -> Ref::
// new_unchecked -> accessors) for both a one-dynamic-tail and a
// two-dynamic-tail schema. Invariant: validate().is_ok() => every Ref accessor
// is total. An AddressSanitizer crash here is a security finding: STOP,
// `cargo fuzz tmin`, report.
fuzz_target!(|data: &[u8]| {
    quasar_lang_fuzz::decode_compact(data);
});
