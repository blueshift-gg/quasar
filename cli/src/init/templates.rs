pub(super) const GITIGNORE: &str = "\
# Build artifacts
/target

# Dependencies
node_modules

# Environment
.env
.env.*

# OS
.DS_Store
";

pub(super) const CARGO_CONFIG: &str = r#"[target.bpfel-unknown-none]
rustflags = [
"--cfg", "feature=\"mem_unaligned\"",
"-C", "linker=sbpf-linker",
"-C", "panic=abort",
"-C", "relocation-model=static",
"-C", "link-arg=--disable-memory-builtins",
"-C", "link-arg=--llvm-args=--bpf-stack-size=4096",
"-C", "link-arg=--export=entrypoint",
"-C", "target-cpu=v2",
"-C", "overflow-checks=off",
]
[alias]
build-bpf = "build -Z build-std=core,alloc --release --target bpfel-unknown-none"
"#;

pub(super) const INSTRUCTIONS_MOD: &str = r#"mod initialize;
pub use initialize::*;
"#;

pub(super) const INSTRUCTION_INITIALIZE: &str = r#"use {
    crate::state::{MyAccount, MyAccountInner},
    quasar_lang::prelude::*,
};

#[derive(Accounts)]
pub struct Initialize {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, address = MyAccount::seeds(payer.address()))]
    pub my_account: Account<MyAccount>,
    pub system_program: Program<SystemProgram>,
}

impl Initialize {
    #[inline(always)]
    pub fn initialize(&mut self, value: u64, bumps: &InitializeBumps) -> Result<(), ProgramError> {
        self.my_account.set_inner(MyAccountInner {
            version: 1,
            authority: *self.payer.address(),
            value,
            bump: bumps.my_account,
            _reserved: [0; 64],
        });
        Ok(())
    }
}
"#;

pub(super) const STATE_RS: &str = r#"use quasar_lang::prelude::*;

#[account(discriminator = 1, set_inner)]
#[seeds(b"my-account", authority: Address)]
pub struct MyAccount {
    pub version: u8,
    pub authority: Address,
    pub value: u64,
    pub bump: u8,
    pub _reserved: [u8; 64],
}
"#;

pub(super) const ERRORS_RS: &str = r#"use quasar_lang::prelude::*;

#[error_code]
pub enum MyError {
    Unauthorized,
}
"#;

pub(super) const TS_TEST_TSCONFIG: &str = r#"{
  "compilerOptions": {
    "esModuleInterop": true,
    "module": "preserve",
    "moduleResolution": "bundler",
    "resolveJsonModule": true,
    "skipLibCheck": true,
    "strict": true,
    "target": "ESNext",
    "types": ["node"]
  },
  "include": ["tests/*.test.ts"]
}
"#;

#[cfg(test)]
mod tests {
    use super::CARGO_CONFIG;

    #[test]
    fn upstream_build_std_is_scoped_to_the_bpf_alias() {
        let config: toml::Value = CARGO_CONFIG.parse().expect("valid generated Cargo config");

        assert!(config.get("unstable").is_none());
        assert_eq!(
            config["alias"]["build-bpf"].as_str(),
            Some("build -Z build-std=core,alloc --release --target bpfel-unknown-none")
        );
    }

    #[test]
    fn upstream_config_uses_supported_linker_flags() {
        // Anchor on the parsed config so this cannot pass vacuously on an
        // empty or broken template, then reject the unsupported flag.
        let config: toml::Value = CARGO_CONFIG.parse().expect("valid generated Cargo config");
        let rustflags = config["target"]["bpfel-unknown-none"]["rustflags"]
            .as_array()
            .expect("rustflags array")
            .iter()
            .map(|flag| flag.as_str().expect("string flag").to_owned())
            .collect::<Vec<_>>();
        assert!(
            !rustflags.is_empty(),
            "generated config must carry linker rustflags"
        );
        assert!(
            !rustflags
                .iter()
                .any(|flag| flag.contains("--disable-expand-memcpy-in-order")),
            "unsupported sbpf-linker flag must not be emitted: {rustflags:?}"
        );
    }
}
