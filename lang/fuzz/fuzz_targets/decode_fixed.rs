#![no_main]

use libfuzzer_sys::fuzz_target;

// Fuzz the fixed `#[instruction]` decode path (length check -> Zc cast ->
// validate_zc -> from_zc) over fully untrusted bytes. An AddressSanitizer
// crash here is a security finding: STOP, `cargo fuzz tmin`, report.
fuzz_target!(|data: &[u8]| {
    quasar_lang_fuzz::decode_fixed(data);
});
