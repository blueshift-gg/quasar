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

pub(super) const CARGO_CONFIG: &str = r#"[unstable]
build-std = ["core", "alloc"]

[target.bpfel-unknown-none]
rustflags = [
"--cfg", "target_os=\"solana\"",
"-A", "explicit_builtin_cfgs_in_flags",
"-C", "linker=sbpf-linker",
"-C", "panic=abort",
"-C", "relocation-model=static",
"-C", "link-arg=--disable-memory-builtins",
"-C", "link-arg=--llvm-args=--bpf-stack-size=4096",
"-C", "link-arg=--disable-expand-memcpy-in-order=true",
"-C", "link-arg=--export=entrypoint",
"-C", "link-arg=--arch=v3",
"-C", "target-cpu=v2",
"-C", "overflow-checks=off",
]
[alias]
build-bpf = "build --release --target bpfel-unknown-none"
"#;

pub(super) const INSTRUCTIONS_MOD: &str = r#"mod initialize;
pub use initialize::*;
"#;

pub(super) const INSTRUCTION_INITIALIZE: &str = r#"use quasar_lang::prelude::*;

#[derive(Accounts)]
pub struct Initialize {
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
}

impl Initialize {
    #[inline(always)]
    pub fn initialize(&self) -> Result<()> {
        Ok(())
    }
}
"#;

pub(super) const STATE_RS: &str = r#"use quasar_lang::prelude::*;

#[account(discriminator = 1)]
pub struct MyAccount {
    pub authority: Address,
    pub value: u64,
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
