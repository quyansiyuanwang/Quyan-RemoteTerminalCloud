use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};
use rtc_agent_packaging::{
    BuildProfile, PackagingCommand, resolve_context_for_profile, run_packaging_command_for_profile,
};
use serde_json::Value;

#[derive(Parser)]
#[command(name = "cargo xtask")]
struct Cli {
    #[arg(long, global = true, conflicts_with = "prod")]
    dev: bool,
    #[arg(long, global = true, conflicts_with = "dev")]
    prod: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run CI checks locally: fmt, clippy, test
    Ci,
    Build,
    Bundle,
    Artifact,
    Package,
    Version(VersionArgs),
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

#[derive(Args)]
struct VersionArgs {
    version: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let build_profile = if cli.dev {
        BuildProfile::Dev
    } else if cli.prod {
        BuildProfile::Prod
    } else {
        BuildProfile::Prod
    };
    let ctx = resolve_context_for_profile(build_profile)?;

    let result = match cli.command {
        Command::Ci => {
            let result = run_ci(&ctx.project_root)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
            return Ok(());
        }
        Command::Build => PackagingCommand::Build,
        Command::Bundle => PackagingCommand::Bundle,
        Command::Artifact => PackagingCommand::Artifact,
        Command::Package => PackagingCommand::Package,
        Command::Version(args) => {
            let result = set_version(&ctx.project_root, &args.version)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
            return Ok(());
        }
        Command::WindowsDesktopBundle(args) => PackagingCommand::WindowsDesktopBundle {
            output_dir: args.output_dir,
            bundles: args.bundles,
            target: args.target,
        },
    };

    let result = run_packaging_command_for_profile(result, build_profile)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

fn run_ci(project_root: &Path) -> Result<Value> {
    println!("═══ cargo fmt --check ═══");
    run_command(project_root, "cargo", &["fmt", "--check"], "fmt check failed")?;

    println!("\n═══ cargo clippy --workspace -- -D warnings ═══");
    run_command(
        project_root,
        "cargo",
        &["clippy", "--workspace", "--", "-D", "warnings"],
        "clippy failed",
    )?;

    println!("\n═══ cargo test --workspace --no-fail-fast ═══");
    run_command(project_root, "cargo", &["test", "--workspace", "--no-fail-fast"], "tests failed")?;

    Ok(serde_json::json!({
        "command": "ci",
        "ok": true,
        "message": "All CI checks passed: fmt, clippy, test.",
    }))
}

fn set_version(project_root: &Path, version: &str) -> Result<Value> {
    validate_version(version)?;

    let current_version = fs::read_to_string(project_root.join("VERSION"))
        .with_context(|| format!("read {}", project_root.join("VERSION").display()))?
        .trim()
        .to_owned();

    let replacements = [
        FileReplacement {
            path: PathBuf::from("VERSION"),
            from: current_version.clone(),
            to: version.to_owned(),
        },
        FileReplacement {
            path: PathBuf::from("Cargo.toml"),
            from: format!("version = \"{current_version}\""),
            to: format!("version = \"{version}\""),
        },
        FileReplacement {
            path: PathBuf::from("apps/rtc-agent-desktop/package.json"),
            from: format!("\"version\": \"{current_version}\""),
            to: format!("\"version\": \"{version}\""),
        },
        FileReplacement {
            path: PathBuf::from("apps/rtc-agent-desktop/package-lock.json"),
            from: format!("\"version\": \"{current_version}\""),
            to: format!("\"version\": \"{version}\""),
        },
        FileReplacement {
            path: PathBuf::from("apps/rtc-agent-desktop/src-tauri/tauri.conf.json"),
            from: format!("\"version\": \"{current_version}\""),
            to: format!("\"version\": \"{version}\""),
        },
        FileReplacement {
            path: PathBuf::from("packaging/windows/wix/RemoteTerminalCloudAgent.wxs"),
            from: format!("Version=\"{current_version}\""),
            to: format!("Version=\"{version}\""),
        },
        FileReplacement {
            path: PathBuf::from("packaging/windows/nsis/agent.nsi"),
            from: format!("!define AGENT_VERSION \"{current_version}\""),
            to: format!("!define AGENT_VERSION \"{version}\""),
        },
        FileReplacement {
            path: PathBuf::from("apps/rtc-agent-desktop/public/mock/status.json"),
            from: format!("\"version\": \"{current_version}\""),
            to: format!("\"version\": \"{version}\""),
        },
        FileReplacement {
            path: PathBuf::from("crates/rtc-agent-runtime/tests/mock_backend.rs"),
            from: format!("\"agentVersion\": \"{current_version}\""),
            to: format!("\"agentVersion\": \"{version}\""),
        },
    ];

    for replacement in replacements {
        replace_in_file(project_root, replacement)?;
    }

    run_command(
        project_root,
        "cargo",
        &["update", "--workspace"],
        "refresh Cargo.lock versions failed",
    )?;
    run_command(
        &project_root.join("apps/rtc-agent-desktop"),
        node_package_manager_command(),
        &["install", "--package-lock-only"],
        "refresh desktop package-lock failed",
    )?;

    Ok(serde_json::json!({
        "command": "version",
        "ok": true,
        "previousVersion": current_version,
        "version": version,
        "message": format!("Workspace version updated to {version}."),
    }))
}

struct FileReplacement {
    path: PathBuf,
    from: String,
    to: String,
}

fn replace_in_file(project_root: &Path, replacement: FileReplacement) -> Result<()> {
    let path = project_root.join(&replacement.path);
    let content = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    if !content.contains(&replacement.from) {
        bail!(
            "expected to find `{}` in {} while updating version",
            replacement.from,
            path.display()
        );
    }
    let updated = content.replacen(&replacement.from, &replacement.to, 1);
    fs::write(&path, updated).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn validate_version(version: &str) -> Result<()> {
    let value = version.trim();
    if value.is_empty() {
        bail!("version cannot be empty");
    }
    let core = value.split_once('-').map(|(head, _)| head).unwrap_or(value);
    let mut parts = core.split('.');
    for _ in 0..3 {
        let part = parts.next().ok_or_else(|| anyhow!("version must look like x.y.z"))?;
        if part.is_empty() || !part.chars().all(|ch| ch.is_ascii_digit()) {
            bail!("version must look like x.y.z");
        }
    }
    if parts.next().is_some() {
        bail!("version must look like x.y.z");
    }
    Ok(())
}

fn run_command(workdir: &Path, program: &str, args: &[&str], error_message: &str) -> Result<()> {
    let mut cmd = ProcessCommand::new(program);
    cmd.current_dir(workdir).args(args).stdout(Stdio::inherit()).stderr(Stdio::inherit());
    let status = cmd.status().with_context(|| {
        format!("{error_message}: failed to start `{program}` in {}", workdir.display())
    })?;
    if !status.success() {
        bail!("{error_message}: exited with status {status}");
    }
    Ok(())
}

fn node_package_manager_command() -> &'static str {
    if cfg!(windows) { "npm.cmd" } else { "npm" }
}
