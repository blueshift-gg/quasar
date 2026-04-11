//! Check evidence — typed proof that constraint handlers emitted the
//! correct runtime checks.
//!
//! Each evidence type is a zero-size struct constructible only via the
//! paired emitter function. `FieldEvidence::validate()` verifies that
//! every declared constraint produced its corresponding evidence —
//! catching "handler emits wrong/incomplete code" bugs at macro
//! expansion time.

/// Proof that an owner+discriminator check was emitted.
pub(super) struct OwnerEvidence(());
impl OwnerEvidence {
    pub(super) fn produced() -> Self {
        Self(())
    }
}

/// Proof that a PDA verification was emitted.
pub(super) struct PdaEvidence(());
impl PdaEvidence {
    pub(super) fn produced() -> Self {
        Self(())
    }
}

/// Proof that a bump resolution was emitted.
pub(super) struct BumpEvidence(());
impl BumpEvidence {
    pub(super) fn produced() -> Self {
        Self(())
    }
}

/// Proof that an init CPI block was emitted.
pub(super) struct InitEvidence(());
impl InitEvidence {
    pub(super) fn produced() -> Self {
        Self(())
    }
}

/// Collected evidence for a single field's codegen.
#[derive(Default)]
pub(super) struct FieldEvidence {
    pub owner: Option<OwnerEvidence>,
    pub pda: Option<PdaEvidence>,
    pub bump: Option<BumpEvidence>,
    pub init: Option<InitEvidence>,
}

impl FieldEvidence {
    /// Validate inter-check invariants. Panics on framework bugs.
    ///
    /// Called at the end of each field's codegen in `process_fields`.
    /// A panic here means a handler emitted incomplete code — this is
    /// a bug in the framework, not in user code.
    pub(super) fn validate(
        &self,
        field_name: &str,
        has_seeds: bool,
        is_init: bool,
    ) {
        if has_seeds && self.pda.is_none() {
            panic!(
                "BUG: field '{}' declares seeds but no PDA verification was emitted",
                field_name,
            );
        }
        if self.pda.is_some() && self.bump.is_none() {
            panic!(
                "BUG: field '{}' has PDA evidence but no bump resolution was emitted",
                field_name,
            );
        }
        if is_init && self.init.is_none() {
            panic!(
                "BUG: field '{}' declares init but no init CPI block was emitted",
                field_name,
            );
        }
    }
}
