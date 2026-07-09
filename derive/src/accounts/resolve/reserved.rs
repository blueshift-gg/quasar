//! Reserved field-name conventions shared across the accounts pipeline.
//!
//! These names carry structural meaning by convention. (Whether they should
//! instead be attribute forms — `#[account(payer)]` — remains an open
//! maintainer decision; centralizing the spellings here is the prerequisite
//! either way.) The planner, event-CPI detection, and diagnostics all read
//! these constants so they can never disagree on the spelling.

/// A field named `payer` is the struct-wide default payer for `init` / `realloc`
/// when a field does not name its own `payer = ...`.
pub(crate) const PAYER_FIELD: &str = "payer";

/// A field named `event_authority` (or one typed `EventAuthority`) enables the
/// event-CPI wiring on the generated accounts struct.
pub(crate) const EVENT_AUTHORITY_FIELD: &str = "event_authority";
