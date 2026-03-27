// ---------------------------------------------------------------------------
// Static templates used by scaffold
// ---------------------------------------------------------------------------

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
"-A", "explicit_builtin_cfgs_in_flags", 
"--cfg", "target_os=\"solana\"",
"--cfg", "feature=\"mem_unaligned\"",
"-C", "linker=sbpf-linker",
"-C", "panic=abort",
"-C", "relocation-model=static",
"-C", "link-arg=--disable-memory-builtins",
"-C", "link-arg=--llvm-args=--bpf-stack-size=4096",
"-C", "link-arg=--disable-expand-memcpy-in-order",
"-C", "link-arg=--export=entrypoint",
"-C", "target-cpu=v2",
]
[alias]
build-bpf = "build --release --target bpfel-unknown-none"
"#;

pub(super) const INSTRUCTIONS_MOD: &str = r#"mod initialize;
pub use initialize::*;
"#;

pub(super) const INSTRUCTION_INITIALIZE: &str = r#"use quasar_lang::prelude::*;

#[derive(Accounts)]
pub struct Initialize<'info> {
    pub payer: &'info mut Signer,
    pub system_program: &'info Program<System>,
}

impl<'info> Initialize<'info> {
    #[inline(always)]
    pub fn initialize(&self) -> Result<(), ProgramError> {
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
    "target": "es2020",
    "module": "commonjs",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "resolveJsonModule": true,
    "types": ["node"]
  },
  "include": ["tests/*.test.ts"]
}
"#;
