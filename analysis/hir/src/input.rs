//! Salsa inputs. `File` is the per-file source-text input — every other query
//! is downstream of it.

use std::sync::Arc;

#[salsa::input(debug)]
pub struct File {
    #[returns(ref)]
    pub text: Arc<str>,
    #[returns(ref)]
    pub path: String,
}
