#![no_main]

use libfuzzer_sys::fuzz_target;

// Stateful model test for the SVM account-region walker. Inputs become
// structured sequences of full accounts and duplicate markers; every prefix
// is checked against a safe Vec oracle through iter, get, and typed parse.
fuzz_target!(|data: &[u8]| {
    quasar_lang_fuzz::remaining_accounts_model(data);
});
