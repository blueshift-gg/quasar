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

/// Proof that has_one / constraint / address checks were emitted.
pub(super) struct FieldCheckEvidence(());
impl FieldCheckEvidence {
    pub(super) fn produced() -> Self {
        Self(())
    }
}

/// Proof that token / ATA / mint validation was emitted.
pub(super) struct TokenValidationEvidence(());
impl TokenValidationEvidence {
    pub(super) fn produced() -> Self {
        Self(())
    }
}

/// Proof that close / sweep codegen was emitted.
pub(super) struct LifecycleEvidence(());
impl LifecycleEvidence {
    pub(super) fn produced() -> Self {
        Self(())
    }
}

/// Proof that realloc codegen was emitted.
pub(super) struct ReallocEvidence(());
impl ReallocEvidence {
    pub(super) fn produced() -> Self {
        Self(())
    }
}

/// Proof that Metaplex metadata / master edition init was emitted.
pub(super) struct MetaplexInitEvidence(());
impl MetaplexInitEvidence {
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
    pub field_check: Option<FieldCheckEvidence>,
    pub token_validation: Option<TokenValidationEvidence>,
    pub lifecycle: Option<LifecycleEvidence>,
    pub realloc: Option<ReallocEvidence>,
    pub metaplex_init: Option<MetaplexInitEvidence>,
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
        attrs: &super::attrs::AccountFieldAttrs,
        has_seeds: bool,
        is_init: bool,
    ) {
        // --- Existing checks (PDA / bump / init) ---
        if has_seeds && self.pda.is_none() {
            panic!(
                "BUG: field '{field_name}' declares seeds but no PDA verification was emitted",
            );
        }
        if self.pda.is_some() && self.bump.is_none() {
            panic!(
                "BUG: field '{field_name}' has PDA evidence but no bump resolution was emitted",
            );
        }
        if is_init && self.init.is_none() {
            panic!(
                "BUG: field '{field_name}' declares init but no init CPI block was emitted",
            );
        }

        // --- NEW: field check evidence (has_one / constraint / address) ---
        let needs_field_check = !attrs.has_ones.is_empty()
            || !attrs.constraints.is_empty()
            || attrs.address.is_some();
        if needs_field_check && self.field_check.is_none() {
            panic!(
                "BUG: field '{field_name}' declares has_one/constraint/address but no checks emitted",
            );
        }

        // --- NEW: lifecycle evidence (close / sweep) ---
        if (attrs.close.is_some() || attrs.sweep.is_some()) && self.lifecycle.is_none() {
            panic!(
                "BUG: field '{field_name}' declares close/sweep but no lifecycle codegen emitted",
            );
        }

        // --- NEW: token validation evidence ---
        // Token validation only applies to non-init fields; init-time
        // validation is embedded in init::gen_init_block.
        let has_token_attrs = attrs.token_mint.is_some()
            || attrs.token_authority.is_some()
            || attrs.associated_token_mint.is_some()
            || attrs.associated_token_authority.is_some()
            || attrs.mint_decimals.is_some()
            || attrs.mint_init_authority.is_some();
        if !is_init && has_token_attrs && self.token_validation.is_none() {
            panic!(
                "BUG: field '{field_name}' declares token::*/mint::* but no validation emitted",
            );
        }

        // --- NEW: realloc evidence ---
        if attrs.realloc.is_some() && self.realloc.is_none() {
            panic!(
                "BUG: field '{field_name}' declares realloc but no realloc codegen emitted",
            );
        }

        // --- NEW: Metaplex init evidence ---
        let has_metaplex = attrs.metadata_name.is_some()
            || attrs.metadata_symbol.is_some()
            || attrs.metadata_uri.is_some()
            || attrs.master_edition_max_supply.is_some();
        if is_init && has_metaplex && self.metaplex_init.is_none() {
            panic!(
                "BUG: field '{field_name}' declares metadata/master_edition but no Metaplex init emitted",
            );
        }
    }
}
