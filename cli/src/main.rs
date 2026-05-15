use clap::Parser;

fn main() {
    quasar_cli::style::init(true);

    let globals = match quasar_cli::config::GlobalConfig::load() {
        Ok(globals) => globals,
        Err(e) => {
            eprintln!("\n  {} {e}", quasar_cli::style::fail(""));
            std::process::exit(e.exit_code());
        }
    };
    quasar_cli::style::init(globals.ui.color);

    // Intercept top-level help before clap so subcommand help still works normally.
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 1 || (args.len() == 2 && matches!(args[1].as_str(), "--help" | "-h" | "help"))
    {
        quasar_cli::print_help();
        return;
    }

    let cli = quasar_cli::Cli::parse();
    if let Err(e) = quasar_cli::run(cli) {
        eprintln!("\n  {} {e}", quasar_cli::style::fail(""));
        std::process::exit(e.exit_code());
    }
}
