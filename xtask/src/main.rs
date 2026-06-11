use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use rtc_agent_packaging::{PackagingCommand, resolve_context, run_packaging_command};

#[derive(Parser)]
#[command(name = "cargo xtask")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Build,
    Bundle,
    Artifact,
    WindowsDesktopBundle(WindowsDesktopBundleArgs),
}

#[derive(Args)]
struct WindowsDesktopBundleArgs {
    #[arg(long)]
    output_dir: Option<PathBuf>,
    #[arg(long, default_value = "nsis")]
    bundles: String,
    #[arg(long)]
    target: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    resolve_context()?;

    let command = match cli.command {
        Command::Build => PackagingCommand::Build,
        Command::Bundle => PackagingCommand::Bundle,
        Command::Artifact => PackagingCommand::Artifact,
        Command::WindowsDesktopBundle(args) => PackagingCommand::WindowsDesktopBundle {
            output_dir: args.output_dir,
            bundles: args.bundles,
            target: args.target,
        },
    };

    let result = run_packaging_command(command)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
