use clap::Parser;

fn main() {
    let globals = cli::config::GlobalConfig::load();
    cli::style::init(globals.ui.color);

    // Intercept top-level help before clap — lets subcommand --help work normally
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 1
        || (args.len() == 2 && matches!(args[1].as_str(), "--help" | "-h" | "help"))
    {
        cli::print_help();
        return;
    }

    let cli = cli::Cli::parse();
    if let Err(e) = cli::run(cli) {
        eprintln!("\n  {} {e}", cli::style::fail(""));
        std::process::exit(1);
    }
}
