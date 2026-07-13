#![no_main]

use libfuzzer_sys::fuzz_target;

// Fuzz the allocating client-side string/vector decoders. The invariant is
// totality over arbitrary bytes: malformed input returns ReadError without a
// panic, unbounded allocation, or invalid UTF-8 value escaping as a string.
fuzz_target!(|data: &[u8]| {
    quasar_lang_fuzz::decode_client(data);
});
