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
    WindowsDownloadWinsw(DownloadWinswArgs),
    WindowsNsisStage(StageArgs),
    WindowsNsisBuild(NsisBuildArgs),
    WindowsMsiStage(StageArgs),
    WindowsMsiBuild(MsiBuildArgs),
}

#[derive(Args)]
struct DownloadWinswArgs {
    #[arg(long)]
    target_exe: Option<PathBuf>,
    #[arg(long, default_value = "v2.12.0")]
    winsw_version: String,
    #[arg(long, default_value_t = false)]
    force: bool,
}

#[derive(Args)]
struct StageArgs {
    #[arg(long)]
    bundle_root: Option<PathBuf>,
    #[arg(long)]
    stage_root: Option<PathBuf>,
    #[arg(long, default_value = "v2.12.0")]
    winsw_version: String,
    #[arg(long, default_value_t = false)]
    include_service: bool,
    #[arg(long, default_value_t = false)]
    force: bool,
}

#[derive(Args)]
struct NsisBuildArgs {
    #[arg(long)]
    build_root: Option<PathBuf>,
    #[arg(long)]
    output_dir: Option<PathBuf>,
    #[arg(long)]
    version: Option<String>,
    #[arg(long)]
    nsis_exe: Option<PathBuf>,
}

#[derive(Args)]
struct MsiBuildArgs {
    #[arg(long)]
    build_root: Option<PathBuf>,
    #[arg(long)]
    output_dir: Option<PathBuf>,
    #[arg(long)]
    wix_exe: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    accept_eula: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let ctx = resolve_context()?;

    let command = match cli.command {
        Command::Build => PackagingCommand::Build,
        Command::Bundle => PackagingCommand::Bundle,
        Command::Artifact => PackagingCommand::Artifact,
        Command::WindowsDownloadWinsw(args) => PackagingCommand::WindowsDownloadWinsw {
            target_exe: args.target_exe.unwrap_or_else(|| {
                ctx.project_root
                    .join("packaging")
                    .join("windows")
                    .join("winsw")
                    .join("RemoteTerminalCloudAgentService.exe")
            }),
            winsw_version: args.winsw_version,
            force: args.force,
        },
        Command::WindowsNsisStage(args) => PackagingCommand::WindowsNsisStage {
            bundle_root: args.bundle_root.unwrap_or_else(|| ctx.bundle_root.clone()),
            stage_root: args.stage_root.unwrap_or_else(|| {
                ctx.bundle_root
                    .join("artifacts")
                    .join("windows")
                    .join("installer-build-root")
            }),
            winsw_version: args.winsw_version,
            include_service: args.include_service,
            force: args.force,
        },
        Command::WindowsNsisBuild(args) => PackagingCommand::WindowsNsisBuild {
            build_root: args.build_root.unwrap_or_else(|| {
                ctx.bundle_root
                    .join("artifacts")
                    .join("windows")
                    .join("installer-build-root")
            }),
            output_dir: args.output_dir.unwrap_or_else(|| {
                ctx.bundle_root
                    .join("artifacts")
                    .join("windows")
                    .join("installer-build-root")
                    .join("artifacts")
                    .join("windows")
                    .join("out")
            }),
            version: args.version,
            nsis_exe: args.nsis_exe,
        },
        Command::WindowsMsiStage(args) => PackagingCommand::WindowsMsiStage {
            bundle_root: args.bundle_root.unwrap_or_else(|| ctx.bundle_root.clone()),
            stage_root: args.stage_root.unwrap_or_else(|| {
                ctx.bundle_root
                    .join("artifacts")
                    .join("windows")
                    .join("msi-build-root")
            }),
            winsw_version: args.winsw_version,
            include_service: args.include_service,
            force: args.force,
        },
        Command::WindowsMsiBuild(args) => PackagingCommand::WindowsMsiBuild {
            build_root: args.build_root.unwrap_or_else(|| {
                ctx.bundle_root
                    .join("artifacts")
                    .join("windows")
                    .join("msi-build-root")
            }),
            output_dir: args.output_dir.unwrap_or_else(|| {
                ctx.bundle_root
                    .join("artifacts")
                    .join("windows")
                    .join("msi-build-root")
                    .join("artifacts")
                    .join("windows")
                    .join("out")
            }),
            wix_exe: args.wix_exe,
            accept_eula: args.accept_eula,
        },
    };

    let result = run_packaging_command(command)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
