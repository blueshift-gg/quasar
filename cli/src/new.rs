use {
    crate::{
        error::{CliError, CliResult},
        style, utils,
    },
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
        return Err(CliError::message(
            "src/lib.rs not found — are you in a Quasar project?",
        ));
    }

    // Create instructions directory if it doesn't exist (minimal template)
    if !instructions_dir.exists() {
        fs::create_dir_all(&instructions_dir).map_err(anyhow::Error::from)?;

        // Wire up `mod instructions;` and `use instructions::*;` in lib.rs
        let lib_content = fs::read_to_string(&lib_path).map_err(anyhow::Error::from)?;
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
            fs::write(&lib_path, updated).map_err(anyhow::Error::from)?;
            println!("  {} src/instructions/", style::success("created"));
        }
    }

    let file_path = instructions_dir.join(format!("{snake}.rs"));
    if file_path.exists() {
        return Err(CliError::message(format!(
            "src/instructions/{snake}.rs already exists"
        )));
    }

    // Write the instruction file
    let pascal = utils::snake_to_pascal(&snake);
    let content = format!(
        r#"use quasar_lang::prelude::*;

#[derive(Accounts)]
pub struct {pascal}<'info> {{
    pub payer: &'info mut Signer,
    pub system_program: &'info Program<SystemProgram>,
}}

impl<'info> {pascal}<'info> {{
    #[inline(always)]
    pub fn {snake}(&self) -> Result<(), ProgramError> {{
        Ok(())
    }}
}}
"#
    );
    fs::write(&file_path, content).map_err(anyhow::Error::from)?;

    // Update mod.rs
    let mod_path = instructions_dir.join("mod.rs");
    let existing_mod = fs::read_to_string(&mod_path).unwrap_or_default();

    if !existing_mod.contains(&format!("mod {snake};")) {
        let new_line = format!("mod {snake};\npub use {snake}::*;\n");
        let updated = format!("{existing_mod}{new_line}");
        fs::write(&mod_path, updated).map_err(anyhow::Error::from)?;
    }

    // Update lib.rs — add instruction to #[program] block
    if lib_path.exists() {
        let lib_content = fs::read_to_string(&lib_path).map_err(anyhow::Error::from)?;
        if let Some(updated) = add_instruction_to_entrypoint(&lib_content, &snake, &pascal) {
            fs::write(&lib_path, updated).map_err(anyhow::Error::from)?;
            println!("  {} src/lib.rs", style::success("updated"));
        }
    }

    println!(
        "  {} src/instructions/{snake}.rs",
        style::success("created")
    );
    println!("  {} src/instructions/mod.rs", style::success("updated"));

    Ok(())
}

/// Find the highest discriminator in the #[program] block and insert
/// a new #[instruction] entry with discriminator = max + 1.
fn add_instruction_to_entrypoint(lib_content: &str, snake: &str, pascal: &str) -> Option<String> {
    // Find the highest existing discriminator
    let mut max_disc: i64 = -1;
    for line in lib_content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("#[instruction(discriminator") {
            if let Some(start) = trimmed.find("= ") {
                if let Some(end) = trimmed[start + 2..].find(')') {
                    if let Ok(n) = trimmed[start + 2..start + 2 + end].trim().parse::<i64>() {
                        if n > max_disc {
                            max_disc = n;
                        }
                    }
                }
            }
        }
    }

    let next_disc = (max_disc + 1) as u64;

    // Find the closing `}}` of the #[program] mod block.
    // Strategy: find the last `}` that closes the program module.
    // We look for the pattern: a line with just `}` or `}}` that ends the mod
    // block. The program block ends with a `}` at indent level 0 after
    // `#[program]`.
    let mut in_program = false;
    let mut program_brace_depth = 0;
    let mut insert_pos = None;

    let mut pos = 0;
    for line in lib_content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("#[program]") {
            in_program = true;
        }

        if in_program {
            for ch in trimmed.chars() {
                if ch == '{' {
                    program_brace_depth += 1;
                } else if ch == '}' {
                    program_brace_depth -= 1;
                    if program_brace_depth == 0 {
                        // This `}` closes the program mod — insert before this line
                        insert_pos = Some(pos);
                        break;
                    }
                }
            }
        }

        if insert_pos.is_some() {
            break;
        }

        pos += line.len() + 1; // +1 for newline
    }

    let insert_pos = insert_pos?;

    let new_entry = format!(
        "\n    #[instruction(discriminator = {next_disc})]\n    pub fn {snake}(ctx: \
         Ctx<{pascal}>) -> Result<(), ProgramError> {{\n        ctx.accounts.{snake}()\n    }}\n"
    );

    let mut result = String::with_capacity(lib_content.len() + new_entry.len());
    result.push_str(&lib_content[..insert_pos]);
    result.push_str(&new_entry);
    result.push_str(&lib_content[insert_pos..]);
    Some(result)
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

    let pascal = utils::snake_to_pascal(&snake);
    let state_path = Path::new("src").join("state.rs");
    let already_exists = state_path.exists();

    if already_exists {
        let existing = fs::read_to_string(&state_path).map_err(anyhow::Error::from)?;

        // Find the highest existing discriminator in state.rs
        let mut max_disc: i64 = 0;
        for line in existing.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("#[account(discriminator") {
                if let Some(start) = trimmed.find("= ") {
                    if let Some(end) = trimmed[start + 2..].find(')') {
                        if let Ok(n) = trimmed[start + 2..start + 2 + end].trim().parse::<i64>() {
                            if n > max_disc {
                                max_disc = n;
                            }
                        }
                    }
                }
            }
        }

        let next_disc = max_disc + 1;
        let new_struct = format!(
            "\n#[account(discriminator = {next_disc})]\npub struct {pascal} {{\n    pub \
             authority: Address,\n}}\n"
        );

        let updated = format!("{existing}{new_struct}");
        fs::write(&state_path, updated).map_err(anyhow::Error::from)?;
    } else {
        let content = format!(
            r#"use quasar_lang::prelude::*;

#[account(discriminator = 1)]
pub struct {pascal} {{
    pub authority: Address,
}}
"#
        );
        fs::write(&state_path, content).map_err(anyhow::Error::from)?;
    }

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

    let pascal = utils::snake_to_pascal(&snake);
    let errors_path = Path::new("src").join("errors.rs");
    let already_exists = errors_path.exists();

    if already_exists {
        let existing = fs::read_to_string(&errors_path).map_err(anyhow::Error::from)?;

        let new_enum = format!("\n#[error_code]\npub enum {pascal} {{\n    Unknown,\n}}\n");

        let updated = format!("{existing}{new_enum}");
        fs::write(&errors_path, updated).map_err(anyhow::Error::from)?;
    } else {
        let content = format!(
            r#"use quasar_lang::prelude::*;

#[error_code]
pub enum {pascal} {{
    Unknown,
}}
"#
        );
        fs::write(&errors_path, content).map_err(anyhow::Error::from)?;
    }

    println!(
        "  {} src/errors.rs ({})",
        style::success(if already_exists { "updated" } else { "created" }),
        pascal,
    );

    Ok(())
}
