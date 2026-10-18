use {
    crate::{
        error::{CliError, CliResult},
        style,
    },
    quasar_schema::snake_to_pascal,
    std::{fs, path::Path},
};

pub fn run_instruction(name: &str) -> CliResult {
    let snake = name.replace('-', "_");

    // Validate: must be a valid Rust identifier (ascii alphanumeric + underscore,
    // not starting with digit)
    if snake.is_empty()
        || snake.starts_with(|c: char| c.is_ascii_digit())
        || !snake.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(CliError::message(format!(
            "invalid instruction name: \"{name}\"\n  must be a valid Rust identifier (e.g. \
             transfer, create_pool)"
        )));
    }

    let instructions_dir = Path::new("src").join("instructions");
    let lib_path = Path::new("src").join("lib.rs");

    if !lib_path.exists() {
        return Err(CliError::message("src/lib.rs not found; are you in a Quasar project?"));
    }

    let original = fs::read_to_string(&lib_path)?;
    add_instruction_to_entrypoint(&original, &snake, &snake_to_pascal(&snake))?;

    // Create instructions directory if it doesn't exist (minimal template)
    if !instructions_dir.exists() {
        fs::create_dir_all(&instructions_dir)?;

        // Wire up `mod instructions;` and `use instructions::*;` in lib.rs
        let lib_content = fs::read_to_string(&lib_path)?;
        if !lib_content.contains("mod instructions;") {
            // Insert after the last `use` or `mod` line at the top
            let insert = "mod instructions;\nuse instructions::*;\n";
            let updated = if let Some(pos) = lib_content.find("#[program]") {
                let mut result = String::with_capacity(lib_content.len() + insert.len());
                result.push_str(&lib_content[..pos]);
                result.push_str(insert);
                result.push('\n');
                result.push_str(&lib_content[pos..]);
                result
            } else {
                format!("{insert}\n{lib_content}")
            };
            fs::write(&lib_path, updated)?;
            println!("  {} src/instructions/", style::success("created"));
        }
    }

    let file_path = instructions_dir.join(format!("{snake}.rs"));
    if file_path.exists() {
        return Err(CliError::message(format!("src/instructions/{snake}.rs already exists")));
    }

    // Write the instruction file
    let pascal = snake_to_pascal(&snake);
    let content = format!(
        r#"use quasar_lang::prelude::*;

#[derive(Accounts)]
pub struct {pascal} {{
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
}}

impl {pascal} {{
    #[inline(always)]
    pub fn {snake}(&self) -> Result<()> {{
        Ok(())
    }}
}}
"#
    );
    fs::write(&file_path, content)?;

    // Update mod.rs
    let mod_path = instructions_dir.join("mod.rs");
    let existing_mod = fs::read_to_string(&mod_path).unwrap_or_default();

    if !existing_mod.contains(&format!("mod {snake};")) {
        let new_line = format!("mod {snake};\npub use {snake}::*;\n");
        let updated = format!("{existing_mod}{new_line}");
        fs::write(&mod_path, updated)?;
    }

    // Add the instruction to the #[program] block in lib.rs.
    if lib_path.exists() {
        let lib_content = fs::read_to_string(&lib_path)?;
        let updated = add_instruction_to_entrypoint(&lib_content, &snake, &pascal)?;
        fs::write(&lib_path, updated)?;
        println!("  {} src/lib.rs", style::success("updated"));
    }

    println!("  {} src/instructions/{snake}.rs", style::success("created"));
    println!("  {} src/instructions/mod.rs", style::success("updated"));

    Ok(())
}

/// Preserve existing discriminators and allocate the next unused value of the
/// same width. Parse Rust so braces inside comments and strings cannot move it.
fn add_instruction_to_entrypoint(
    lib_content: &str,
    snake: &str,
    pascal: &str,
) -> Result<String, CliError> {
    let file = syn::parse_file(lib_content).map_err(|e| CliError::message(e.to_string()))?;
    let module = file
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Mod(module) if module.attrs.iter().any(|a| a.path().is_ident("program")) => {
                Some(module)
            }
            _ => None,
        })
        .ok_or_else(|| CliError::message("no inline #[program] module found in src/lib.rs"))?;
    let (brace, items) = module
        .content
        .as_ref()
        .ok_or_else(|| CliError::message("#[program] needs an inline module"))?;
    let mut used = Vec::new();
    for item in items {
        if let syn::Item::Fn(handler) = item {
            if handler.sig.ident == snake {
                return Err(CliError::message("instruction already exists"));
            }
            for attr in &handler.attrs {
                if !attr.path().is_ident("instruction") {
                    continue;
                }
                let expression: syn::Expr =
                    attr.parse_args().map_err(|e| CliError::message(e.to_string()))?;
                let literals = match expression {
                    syn::Expr::Array(array) => array.elems.into_iter().collect(),
                    expr => vec![expr],
                };
                let bytes: Vec<u8> = literals
                    .into_iter()
                    .map(|expr| {
                        if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(lit), .. }) = expr {
                            lit.base10_parse().map_err(|e| CliError::message(e.to_string()))
                        } else {
                            Err(CliError::message(
                                "instruction discriminators must be byte literals",
                            ))
                        }
                    })
                    .collect::<Result<_, _>>()?;
                if bytes.is_empty() {
                    return Err(CliError::message("empty instruction discriminator"));
                }
                used.push(bytes);
            }
        }
    }
    let width = used.first().map_or(1, Vec::len);
    if used.iter().any(|bytes| bytes.len() != width) {
        return Err(CliError::message("instruction discriminators have different widths"));
    }
    let mut next = vec![0u8; width];
    while used.contains(&next) {
        let mut carry = true;
        for byte in &mut next {
            (*byte, carry) = byte.overflowing_add(1);
            if !carry {
                break;
            }
        }
        if carry {
            return Err(CliError::message("all instruction discriminators are in use"));
        }
    }
    let location = brace.span.close().start();
    let insert_pos =
        lib_content.split_inclusive('\n').take(location.line - 1).map(str::len).sum::<usize>()
            + location.column;
    let new_entry = format!(
        "\n    #[instruction({next:?})]\n    pub fn {snake}(ctx: Ctx<{pascal}>) -> Result<()> {{\n        ctx.accounts.{snake}()\n    }}\n"
    );
    let mut result = lib_content.to_owned();
    result.insert_str(insert_pos, &new_entry);
    Ok(result)
}

fn parse_discriminator(line: &str) -> Option<i64> {
    let line = line.trim();
    if !(line.starts_with("#[instruction(") || line.starts_with("#[account(")) {
        return None;
    }

    let (_, after_name) = line.split_once("discriminator")?;
    let (_, after_eq) = after_name.split_once('=')?;
    let digits: String = after_eq.trim_start().chars().take_while(|c| c.is_ascii_digit()).collect();

    if digits.is_empty() {
        return None;
    }

    digits.parse().ok()
}

pub fn run_state(name: &str) -> CliResult {
    let snake = name.replace('-', "_");

    if snake.is_empty()
        || snake.starts_with(|c: char| c.is_ascii_digit())
        || !snake.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(CliError::message(format!(
            "invalid state name: \"{name}\"\n  must be a valid Rust identifier (e.g. vault, \
             user_profile)"
        )));
    }

    let pascal = snake_to_pascal(&snake);
    let state_path = Path::new("src").join("state.rs");
    let already_exists = state_path.exists();

    if already_exists {
        let existing = fs::read_to_string(&state_path)?;

        let mut max_disc: i64 = 0;
        for line in existing.lines() {
            if let Some(n) = parse_discriminator(line)
                && n > max_disc
            {
                max_disc = n;
            }
        }

        let next_disc = max_disc + 1;
        let new_struct = format!(
            "\n#[account(discriminator = {next_disc})]\npub struct {pascal} {{\n    pub \
             authority: Address,\n}}\n"
        );

        let updated = format!("{existing}{new_struct}");
        fs::write(&state_path, updated)?;
    } else {
        let content = format!(
            r#"use quasar_lang::prelude::*;

#[account(discriminator = 1)]
pub struct {pascal} {{
    pub authority: Address,
}}
"#
        );
        fs::write(&state_path, content)?;
    }

    wire_module("state")?;
    println!(
        "  {} src/state.rs ({})",
        style::success(if already_exists { "updated" } else { "created" }),
        pascal,
    );

    Ok(())
}

pub fn run_error(name: &str) -> CliResult {
    let snake = name.replace('-', "_");

    if snake.is_empty()
        || snake.starts_with(|c: char| c.is_ascii_digit())
        || !snake.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(CliError::message(format!(
            "invalid error name: \"{name}\"\n  must be a valid Rust identifier (e.g. vault_error, \
             access_error)"
        )));
    }

    let pascal = snake_to_pascal(&snake);
    let errors_path = Path::new("src").join("errors.rs");
    let already_exists = errors_path.exists();

    if already_exists {
        let existing = fs::read_to_string(&errors_path)?;

        let new_enum = format!("\n#[error_code]\npub enum {pascal} {{\n    Unknown,\n}}\n");

        let updated = format!("{existing}{new_enum}");
        fs::write(&errors_path, updated)?;
    } else {
        let content = format!(
            r#"use quasar_lang::prelude::*;

#[error_code]
pub enum {pascal} {{
    Unknown,
}}
"#
        );
        fs::write(&errors_path, content)?;
    }

    wire_module("errors")?;
    println!(
        "  {} src/errors.rs ({})",
        style::success(if already_exists { "updated" } else { "created" }),
        pascal,
    );

    Ok(())
}

fn wire_module(name: &str) -> CliResult {
    let path = Path::new("src/lib.rs");
    let source = fs::read_to_string(path)?;
    let declaration = format!("mod {name};");
    if !source.lines().any(|line| line.trim() == declaration) {
        fs::write(path, format!("{source}\n{declaration}\n"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::{add_instruction_to_entrypoint, parse_discriminator, run_instruction},
        std::sync::{Mutex, OnceLock},
    };

    static CWD_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    #[test]
    fn discriminator_parser_accepts_space_or_no_space() {
        assert_eq!(parse_discriminator("#[instruction(discriminator = 7)]"), Some(7));
        assert_eq!(parse_discriminator("#[account(discriminator=8)]"), Some(8));
    }

    #[test]
    fn instruction_insert_preserves_existing_discriminators() {
        let source = r#"#[program]
mod demo {
    use super::*;

    #[instruction(3)]
    pub fn initialize(ctx: Ctx<Initialize>) -> Result<()> {
        ctx.accounts.initialize()
    }
}
"#;

        let updated = add_instruction_to_entrypoint(source, "settle", "Settle")
            .expect("program block should be updated");

        assert!(updated.contains("#[instruction([0])]"));
        assert!(updated.contains("pub fn settle(ctx: Ctx<Settle>)"));
    }

    #[test]
    fn add_instruction_generates_owned_account_wrappers() {
        let _guard = CWD_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let src_dir = temp.path().join("src");
        std::fs::create_dir(&src_dir).unwrap();
        std::fs::write(
            src_dir.join("lib.rs"),
            r#"use quasar_lang::prelude::*;

#[program]
mod demo {
    use super::*;
}
"#,
        )
        .unwrap();

        std::env::set_current_dir(temp.path()).unwrap();
        let result = run_instruction("create_pool");
        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap();

        let generated =
            std::fs::read_to_string(src_dir.join("instructions/create_pool.rs")).unwrap();
        assert!(generated.contains("pub struct CreatePool {"));
        assert!(generated.contains("pub payer: Signer,"));
        assert!(generated.contains("pub system_program: Program<SystemProgram>,"));
        assert!(!generated.contains("'info"));
        assert!(!generated.contains("&'info"));
    }
}
