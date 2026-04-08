use quasar_idl::lint;

#[test]
fn lint_report_empty_for_constrained_program() {
    let src = r#"
        declare_id!("11111111111111111111111111111111");

        #[program]
        mod test_program {
            use super::*;

            #[instruction(discriminator = [1])]
            pub fn approve(ctx: Ctx<Approve>) -> Result<(), ProgramError> {
                Ok(())
            }
        }

        #[derive(Accounts)]
        pub struct Approve<'info> {
            pub authority: Signer,
            #[account(mut, has_one = authority)]
            pub vault: Account<Vault<'info>>,
        }

        #[account(discriminator = 1)]
        pub struct Vault {
            pub authority: Address,
            pub balance: u64,
        }
    "#;

    let parsed = quasar_idl::parser::parse_program_from_source(src);
    let report = lint::run_lint(&parsed, &lint::LintConfig::default());
    assert!(
        report.diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        report.diagnostics
    );
}
