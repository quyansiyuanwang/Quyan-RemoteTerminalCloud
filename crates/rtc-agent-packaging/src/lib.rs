use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

use anyhow::{Context, Result, anyhow, bail};
use flate2::Compression;
use flate2::write::GzEncoder;
use reqwest::blocking::Client;
use rtc_agent_config::RELEASE_SERVER_BASE_URL;
use serde::Serialize;
use tar::Builder as TarBuilder;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use which::which;
use zip::CompressionMethod;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

const DEFAULT_WINSW_VERSION: &str = "v2.12.0";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagingActionResult {
    pub command: String,
    pub ok: bool,
    pub message: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct PackagingContext {
    pub project_root: PathBuf,
    pub version: String,
    pub target_platform: String,
    pub target_arch: String,
    pub os_name: String,
    pub arch_name: String,
    pub release_root: PathBuf,
    pub bundle_root: PathBuf,
    pub platform_out_root: PathBuf,
    pub stage_root: PathBuf,
    pub archive_base_name: String,
}

#[derive(Debug, Clone)]
pub enum PackagingCommand {
    Build,
    Bundle,
    Artifact,
    WindowsDownloadWinsw {
        target_exe: PathBuf,
        winsw_version: String,
        force: bool,
    },
    WindowsNsisStage {
        bundle_root: PathBuf,
        stage_root: PathBuf,
        winsw_version: String,
        include_service: bool,
        force: bool,
    },
    WindowsNsisBuild {
        build_root: PathBuf,
        output_dir: PathBuf,
        version: Option<String>,
        nsis_exe: Option<PathBuf>,
    },
    WindowsMsiStage {
        bundle_root: PathBuf,
        stage_root: PathBuf,
        winsw_version: String,
        include_service: bool,
        force: bool,
    },
    WindowsMsiBuild {
        build_root: PathBuf,
        output_dir: PathBuf,
        wix_exe: Option<PathBuf>,
        accept_eula: bool,
    },
}

pub fn resolve_context() -> Result<PackagingContext> {
    let project_root = repo_root()?;
    let version = read_version(&project_root)?;

    let target_platform = normalize_target_platform(env_or("RTC_TARGET_PLATFORM", env::consts::OS));
    let target_arch = normalize_target_arch(env_or("RTC_TARGET_ARCH", env::consts::ARCH));

    let os_name = match target_platform.as_str() {
        "win32" => "windows".to_owned(),
        value => value.to_owned(),
    };
    let arch_name = match target_arch.as_str() {
        "x64" => "x86_64".to_owned(),
        value => value.to_owned(),
    };

    let release_root = project_root.join("release");
    let bundle_root = release_root.join(format!("remote-terminal-cloud-agent-{version}"));
    let platform_out_root =
        release_root.join("artifacts").join(format!("{target_platform}-{target_arch}"));
    let stage_root = platform_out_root.join(format!("remote-terminal-cloud-agent-{version}"));
    let archive_base_name =
        format!("remote-terminal-cloud-agent-{version}-{target_platform}-{target_arch}");

    Ok(PackagingContext {
        project_root,
        version,
        target_platform,
        target_arch,
        os_name,
        arch_name,
        release_root,
        bundle_root,
        platform_out_root,
        stage_root,
        archive_base_name,
    })
}

pub fn run_packaging_command(command: PackagingCommand) -> Result<PackagingActionResult> {
    let ctx = resolve_context()?;
    match command {
        PackagingCommand::Build => build_command(&ctx),
        PackagingCommand::Bundle => bundle_command(&ctx),
        PackagingCommand::Artifact => artifact_command(&ctx),
        PackagingCommand::WindowsDownloadWinsw { target_exe, winsw_version, force } => {
            download_winsw_command(&ctx, &target_exe, &winsw_version, force)
        }
        PackagingCommand::WindowsNsisStage {
            bundle_root,
            stage_root,
            winsw_version,
            include_service,
            force,
        } => windows_nsis_stage_command(
            &ctx,
            &bundle_root,
            &stage_root,
            &winsw_version,
            include_service,
            force,
        ),
        PackagingCommand::WindowsNsisBuild { build_root, output_dir, version, nsis_exe } => {
            windows_nsis_build_command(&ctx, &build_root, &output_dir, version, nsis_exe)
        }
        PackagingCommand::WindowsMsiStage {
            bundle_root,
            stage_root,
            winsw_version,
            include_service,
            force,
        } => windows_msi_stage_command(
            &ctx,
            &bundle_root,
            &stage_root,
            &winsw_version,
            include_service,
            force,
        ),
        PackagingCommand::WindowsMsiBuild { build_root, output_dir, wix_exe, accept_eula } => {
            windows_msi_build_command(&ctx, &build_root, &output_dir, wix_exe, accept_eula)
        }
    }
}

fn build_command(ctx: &PackagingContext) -> Result<PackagingActionResult> {
    let output_dir = build_bin_dir(ctx);
    fs::create_dir_all(&output_dir).with_context(|| format!("create {}", output_dir.display()))?;

    build_cli_binary(
        ctx,
        "rtc-agentd",
        &output_dir.join(binary_file_name(ctx, "rtc-agent")),
        false,
    )?;
    build_cli_binary(
        ctx,
        "rtc-agent-installer",
        &output_dir.join(binary_file_name(ctx, "rtc-agent-installer")),
        false,
    )?;
    build_desktop_binary(ctx, &output_dir)?;
    copy_compatibility_manager_binary(
        &output_dir.join(binary_file_name(ctx, "rtc-agent-desktop")),
        &output_dir.join(binary_file_name(ctx, "rtc-agent-manager")),
    )?;

    let mut details = BTreeMap::new();
    details.insert("outputDir".into(), output_dir.display().to_string());
    Ok(success("build", "Rust binaries built successfully.", details))
}

fn bundle_command(ctx: &PackagingContext) -> Result<PackagingActionResult> {
    if ctx.release_root.exists() {
        fs::remove_dir_all(&ctx.release_root)
            .with_context(|| format!("remove {}", ctx.release_root.display()))?;
    }
    fs::create_dir_all(ctx.bundle_root.join("bin"))
        .with_context(|| format!("create {}", ctx.bundle_root.join("bin").display()))?;

    build_command(ctx)?;

    let source_bin_dir = build_bin_dir(ctx);
    for name in [
        binary_file_name(ctx, "rtc-agent"),
        binary_file_name(ctx, "rtc-agent-manager"),
        binary_file_name(ctx, "rtc-agent-desktop"),
        binary_file_name(ctx, "rtc-agent-installer"),
    ] {
        copy_file(&source_bin_dir.join(&name), &ctx.bundle_root.join("bin").join(&name))?;
    }

    copy_tree(&ctx.project_root.join("packaging"), &ctx.bundle_root.join("packaging"))?;
    copy_tree(&ctx.project_root.join("docs"), &ctx.bundle_root.join("docs"))?;
    copy_tree(&ctx.project_root.join("apps"), &ctx.bundle_root.join("apps"))?;
    copy_tree(&ctx.project_root.join("crates"), &ctx.bundle_root.join("crates"))?;
    copy_tree(&ctx.project_root.join("xtask"), &ctx.bundle_root.join("xtask"))?;

    for file_name in ["VERSION", "Cargo.toml", "Cargo.lock", "AGENTS.md", "README.md", "rustfmt.toml"]
    {
        let src = ctx.project_root.join(file_name);
        if src.is_file() {
            copy_file(&src, &ctx.bundle_root.join(file_name))?;
        }
    }

    for platform in ["windows", "linux", "macos"] {
        fs::create_dir_all(ctx.bundle_root.join("artifacts").join(platform)).with_context(
            || format!("create {}", ctx.bundle_root.join("artifacts").join(platform).display()),
        )?;
    }

    write_json(
        &ctx.bundle_root.join("bundle.json"),
        serde_json::json!({
            "name": "rtc-agent",
            "version": ctx.version,
            "binary": slash_join(["bin", &binary_file_name(ctx, "rtc-agent")]),
            "managerBinary": slash_join(["bin", &binary_file_name(ctx, "rtc-agent-manager")]),
            "desktopBinary": slash_join(["bin", &binary_file_name(ctx, "rtc-agent-desktop")]),
            "installerBinary": slash_join(["bin", &binary_file_name(ctx, "rtc-agent-installer")]),
        }),
    )?;

    let mut details = BTreeMap::new();
    details.insert("bundleRoot".into(), ctx.bundle_root.display().to_string());
    Ok(success("bundle", "Rust release bundle assembled.", details))
}

fn artifact_command(ctx: &PackagingContext) -> Result<PackagingActionResult> {
    if !ctx.bundle_root.exists() {
        bundle_command(ctx)?;
    }

    if ctx.platform_out_root.exists() {
        fs::remove_dir_all(&ctx.platform_out_root)
            .with_context(|| format!("remove {}", ctx.platform_out_root.display()))?;
    }
    fs::create_dir_all(&ctx.stage_root)
        .with_context(|| format!("create {}", ctx.stage_root.display()))?;

    copy_tree(&ctx.bundle_root, &ctx.stage_root)?;

    write_json(
        &ctx.stage_root.join("ARTIFACT-INFO.json"),
        serde_json::json!({
            "generatedAt": now_rfc3339()?,
            "version": ctx.version,
            "targetPlatform": ctx.target_platform,
            "targetArch": ctx.target_arch,
            "archiveFile": archive_file_name(ctx),
            "nativeInstallerFile": native_installer_file_name(ctx),
            "binaryPath": slash_join(["bin", &binary_file_name(ctx, "rtc-agent")]),
            "managerBinaryPath": slash_join(["bin", &binary_file_name(ctx, "rtc-agent-manager")]),
            "desktopBinaryPath": slash_join(["bin", &binary_file_name(ctx, "rtc-agent-desktop")]),
            "installerBinaryPath": slash_join(["bin", &binary_file_name(ctx, "rtc-agent-installer")]),
            "startCommand": start_command(ctx),
        }),
    )?;

    let readme = [
        "Remote Terminal Cloud Agent platform artifact",
        "",
        &format!("Version: {}", ctx.version),
        &format!("Platform: {}", ctx.target_platform),
        &format!("Architecture: {}", ctx.target_arch),
        &format!("Server base URL: {RELEASE_SERVER_BASE_URL}"),
        "",
        "This artifact contains:",
        "- bin/ Rust agent, installer, desktop, and compatibility manager binaries",
        "- packaging/ platform service and installer templates",
        "- docs/ deployment and packaging notes",
    ]
    .join("\n");
    fs::write(ctx.stage_root.join("README.txt"), readme)
        .with_context(|| format!("write {}", ctx.stage_root.join("README.txt").display()))?;

    create_archive(ctx)?;
    build_native_installer(ctx)?;

    let mut details = BTreeMap::new();
    details.insert("stageRoot".into(), ctx.stage_root.display().to_string());
    details.insert(
        "archive".into(),
        ctx.platform_out_root.join(archive_file_name(ctx)).display().to_string(),
    );
    Ok(success("artifact", "Rust platform artifact assembled.", details))
}

fn download_winsw_command(
    _ctx: &PackagingContext,
    target_exe: &Path,
    winsw_version: &str,
    force: bool,
) -> Result<PackagingActionResult> {
    download_winsw_executable(target_exe, winsw_version, force)?;
    let mut details = BTreeMap::new();
    details.insert("targetExe".into(), target_exe.display().to_string());
    Ok(success("windows-download-winsw", "WinSW downloaded.", details))
}

fn windows_nsis_stage_command(
    ctx: &PackagingContext,
    bundle_root: &Path,
    stage_root: &Path,
    winsw_version: &str,
    include_service: bool,
    force: bool,
) -> Result<PackagingActionResult> {
    ensure_bundle_exists(ctx, bundle_root)?;
    validate_windows_bundle_root(bundle_root, true, true)?;
    prepare_clean_directory(stage_root, force)?;

    for dir in [
        stage_root.join("bin"),
        stage_root.join("packaging").join("windows").join("nsis"),
        stage_root.join("service"),
        stage_root.join("artifacts").join("windows").join("out"),
    ] {
        fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    }

    stage_windows_payload(bundle_root, stage_root, true, include_service)?;
    if include_service {
        download_winsw_executable(
            &stage_root.join("service").join("RemoteTerminalCloudAgentService.exe"),
            winsw_version,
            true,
        )?;
    }

    let output_dir = stage_root.join("artifacts").join("windows").join("out");
    write_json(
        &stage_root.join("NSIS-INPUTS.json"),
        serde_json::json!({
            "generatedAt": now_rfc3339()?,
            "agentBundleRoot": bundle_root.display().to_string(),
            "winSWVersion": winsw_version,
            "serviceMode": include_service,
            "stageRoot": stage_root.display().to_string(),
            "outputDir": output_dir.display().to_string(),
            "buildExample": xtask_command_line("windows-nsis-build", &[("--build-root", Some(stage_root))]),
        }),
    )?;

    let mut readme_lines = vec![
        "Remote Terminal Cloud Agent - Windows NSIS build root".to_owned(),
        "".into(),
        "This directory is ready to be used with cargo xtask.".into(),
        "".into(),
        "Build command:".into(),
        xtask_command_line("windows-nsis-build", &[("--build-root", Some(stage_root))]),
        "".into(),
        "Default mode:".into(),
        "- Installs and launches rtc-agent-desktop as the primary background app".into(),
        "- Service wrapper files are optional and excluded unless --include-service is used".into(),
        "".into(),
        "Key paths:".into(),
        "- bin\\rtc-agent.exe".into(),
        "- bin\\rtc-agent-manager.exe".into(),
        "- bin\\rtc-agent-desktop.exe".into(),
        "- bin\\rtc-agent-installer.exe".into(),
        "- packaging\\windows\\nsis\\agent.nsi".into(),
        "- artifacts\\windows\\out".into(),
    ];
    if include_service {
        readme_lines.push("- service\\RemoteTerminalCloudAgentService.exe".into());
        readme_lines.push("- service\\RemoteTerminalCloudAgentService.xml".into());
    }
    fs::write(stage_root.join("README.txt"), readme_lines.join("\n"))
        .with_context(|| format!("write {}", stage_root.join("README.txt").display()))?;

    let mut details = BTreeMap::new();
    details.insert("stageRoot".into(), stage_root.display().to_string());
    details.insert("outputDir".into(), output_dir.display().to_string());
    Ok(success("windows-nsis-stage", "Windows NSIS staging prepared.", details))
}

fn windows_nsis_build_command(
    ctx: &PackagingContext,
    build_root: &Path,
    output_dir: &Path,
    version: Option<String>,
    nsis_exe: Option<PathBuf>,
) -> Result<PackagingActionResult> {
    validate_windows_build_root(build_root, true, true)?;
    fs::create_dir_all(output_dir).with_context(|| format!("create {}", output_dir.display()))?;

    let resolved_version = version
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .or_else(|| load_version_from_build_root(build_root))
        .unwrap_or_else(|| ctx.version.clone());

    let nsis_path = find_nsis_executable(nsis_exe.as_deref())?;
    let script = build_root.join("packaging").join("windows").join("nsis").join("agent.nsi");
    run_streaming_command(
        Some(build_root),
        &nsis_path,
        &[
            format!("/DAGENT_BUILD_ROOT={}", build_root.display()),
            format!("/DAGENT_OUTPUT_DIR={}", output_dir.display()),
            format!("/DAGENT_VERSION={resolved_version}"),
            script.display().to_string(),
        ],
    )?;

    let mut details = BTreeMap::new();
    details.insert("outputDir".into(), output_dir.display().to_string());
    details.insert("nsisExe".into(), nsis_path.display().to_string());
    Ok(success("windows-nsis-build", "Windows NSIS installer built.", details))
}

fn windows_msi_stage_command(
    ctx: &PackagingContext,
    bundle_root: &Path,
    stage_root: &Path,
    winsw_version: &str,
    include_service: bool,
    force: bool,
) -> Result<PackagingActionResult> {
    ensure_bundle_exists(ctx, bundle_root)?;
    validate_windows_bundle_root(bundle_root, true, true)?;
    prepare_clean_directory(stage_root, force)?;

    for dir in [
        stage_root.join("bin"),
        stage_root.join("packaging").join("windows").join("wix"),
        stage_root.join("service"),
        stage_root.join("artifacts").join("windows").join("out"),
    ] {
        fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    }

    stage_windows_payload(bundle_root, stage_root, false, include_service)?;
    if include_service {
        download_winsw_executable(
            &stage_root.join("service").join("RemoteTerminalCloudAgentService.exe"),
            winsw_version,
            true,
        )?;
    }

    let output_dir = stage_root.join("artifacts").join("windows").join("out");
    write_json(
        &stage_root.join("MSI-INPUTS.json"),
        serde_json::json!({
            "generatedAt": now_rfc3339()?,
            "agentBundleRoot": bundle_root.display().to_string(),
            "winSWVersion": winsw_version,
            "serviceMode": include_service,
            "stageRoot": stage_root.display().to_string(),
            "outputDir": output_dir.display().to_string(),
            "buildExample": xtask_command_line("windows-msi-build", &[
                ("--build-root", Some(stage_root)),
                ("--output-dir", Some(&output_dir)),
            ]),
        }),
    )?;

    if include_service {
        let service_readme = [
            "This folder contains the optional service wrapper payload for MSI packaging.",
            "Bundled files:",
            "- RemoteTerminalCloudAgentService.exe",
            "- RemoteTerminalCloudAgentService.xml",
        ]
        .join("\n");
        fs::write(stage_root.join("service").join("README.txt"), service_readme).with_context(
            || format!("write {}", stage_root.join("service").join("README.txt").display()),
        )?;
    }

    let mut stage_readme = vec![
        "Remote Terminal Cloud Agent - Windows MSI build root".to_owned(),
        "".into(),
        "This directory is ready to be used as AgentBuildRoot for WiX.".into(),
        "".into(),
        "Build command:".into(),
        xtask_command_line(
            "windows-msi-build",
            &[("--build-root", Some(stage_root)), ("--output-dir", Some(&output_dir))],
        ),
        "".into(),
        "Default mode:".into(),
        "- Installs and launches rtc-agent-desktop as the primary background app".into(),
        "- Service wrapper files are optional and excluded unless --include-service is used".into(),
        "".into(),
        "Key paths:".into(),
        "- bin\\rtc-agent.exe".into(),
        "- bin\\rtc-agent-manager.exe".into(),
        "- bin\\rtc-agent-desktop.exe".into(),
        "- bin\\rtc-agent-installer.exe".into(),
        "- packaging\\windows\\agent.config.json".into(),
        "- packaging\\windows\\wix\\RemoteTerminalCloudAgent.wxs".into(),
    ];
    if include_service {
        stage_readme.push("- service\\RemoteTerminalCloudAgentService.exe".into());
        stage_readme.push("- service\\RemoteTerminalCloudAgentService.xml".into());
    }
    fs::write(stage_root.join("README.txt"), stage_readme.join("\n"))
        .with_context(|| format!("write {}", stage_root.join("README.txt").display()))?;

    let mut details = BTreeMap::new();
    details.insert("stageRoot".into(), stage_root.display().to_string());
    details.insert("outputDir".into(), output_dir.display().to_string());
    Ok(success("windows-msi-stage", "Windows MSI staging prepared.", details))
}

fn windows_msi_build_command(
    _ctx: &PackagingContext,
    build_root: &Path,
    output_dir: &Path,
    wix_exe: Option<PathBuf>,
    accept_eula: bool,
) -> Result<PackagingActionResult> {
    validate_windows_build_root(build_root, false, true)?;
    fs::create_dir_all(output_dir).with_context(|| format!("create {}", output_dir.display()))?;

    let wix_path = find_executable(wix_exe.as_deref(), &["wix.exe", "wix"])
        .context("wix.exe not found. Install WiX Toolset CLI and ensure it is on PATH")?;
    let (wix_version, wix_major) = detect_wix_version(&wix_path)?;

    let mut extension_name = "WixToolset.UI.wixext".to_owned();
    if let Some(version) = wix_version.as_ref() {
        extension_name.push('/');
        extension_name.push_str(version);
    }
    run_streaming_command(None, &wix_path, &["extension".into(), "add".into(), extension_name, "--global".into()])?;

    let msi_path = output_dir.join("RemoteTerminalCloudAgent.msi");
    let mut wix_args = vec!["build".to_owned()];
    if accept_eula && wix_major.unwrap_or_default() >= 7 {
        wix_args.push("--acceptEula".into());
        wix_args.push("yes".into());
    }
    wix_args.extend([
        "-ext".into(),
        "WixToolset.UI.wixext".into(),
        "-d".into(),
        format!("AgentBuildRoot={}", build_root.display()),
        build_root
            .join("packaging")
            .join("windows")
            .join("wix")
            .join("RemoteTerminalCloudAgent.wxs")
            .display()
            .to_string(),
        "-out".into(),
        msi_path.display().to_string(),
    ]);
    run_streaming_command(Some(build_root), &wix_path, &wix_args)?;

    let mut details = BTreeMap::new();
    if let Some(version) = wix_version {
        details.insert("wixVersion".into(), version);
    }
    details.insert("output".into(), msi_path.display().to_string());
    Ok(success("windows-msi-build", "Windows MSI built.", details))
}

fn build_cli_binary(
    ctx: &PackagingContext,
    package_name: &str,
    output_path: &Path,
    windows_gui: bool,
) -> Result<()> {
    let temp_output = output_path.with_extension(temp_extension(output_path));
    if temp_output.exists() {
        fs::remove_file(&temp_output)
            .with_context(|| format!("remove {}", temp_output.display()))?;
    }

    let mut cmd = Command::new("cargo");
    cmd.current_dir(&ctx.project_root)
        .arg("build")
        .arg("--release")
        .arg("-p")
        .arg(package_name)
        .env("RTC_AGENT_SERVER_BASE_URL", RELEASE_SERVER_BASE_URL)
        .env("CARGO_TARGET_DIR", ctx.project_root.join("target"));

    if ctx.os_name != env::consts::OS || normalize_host_arch(env::consts::ARCH) != ctx.target_arch {
        let target = rust_target_triple(ctx)?;
        cmd.arg("--target").arg(&target);
    }

    if cfg!(windows) && windows_gui {
        cmd.env("RUSTFLAGS", "-C link-args=/SUBSYSTEM:WINDOWS");
    }

    run_command(&mut cmd, "cargo build failed")?;

    let binary_name = package_binary_file_name(ctx, package_name);
    let built_path = cargo_binary_output_path(ctx, package_name)?;
    copy_file(&built_path, &temp_output)?;
    replace_file(&temp_output, output_path).with_context(|| {
        format!(
            "failed to move {} into place as {} for package {binary_name}",
            temp_output.display(),
            output_path.display()
        )
    })?;
    Ok(())
}

fn build_desktop_binary(ctx: &PackagingContext, output_dir: &Path) -> Result<()> {
    let desktop_dir = ctx.project_root.join("apps").join("rtc-agent-desktop");
    let mut npm = Command::new(node_package_manager_command());
    npm.current_dir(&desktop_dir).arg("run").arg("build");
    run_command(&mut npm, "desktop frontend build failed")?;

    let mut cargo = Command::new("cargo");
    cargo
        .current_dir(&ctx.project_root)
        .arg("build")
        .arg("--release")
        .arg("-p")
        .arg("rtc-agent-desktop")
        .env("RTC_AGENT_SERVER_BASE_URL", RELEASE_SERVER_BASE_URL)
        .env("CARGO_TARGET_DIR", ctx.project_root.join("target"));
    run_command(&mut cargo, "desktop Rust build failed")?;

    let source = ctx
        .project_root
        .join("target")
        .join("release")
        .join(binary_file_name(ctx, "rtc-agent-desktop"));
    let target = output_dir.join(binary_file_name(ctx, "rtc-agent-desktop"));
    copy_file(&source, &target)?;
    Ok(())
}

fn node_package_manager_command() -> &'static str {
    if cfg!(windows) { "npm.cmd" } else { "npm" }
}

fn copy_compatibility_manager_binary(source: &Path, target: &Path) -> Result<()> {
    copy_file(source, target)
}

fn build_bin_dir(ctx: &PackagingContext) -> PathBuf {
    ctx.project_root
        .join("build")
        .join("bin")
        .join(format!("{}-{}", ctx.target_platform, ctx.target_arch))
}

fn binary_file_name(ctx: &PackagingContext, stem: &str) -> String {
    if ctx.target_platform == "win32" { format!("{stem}.exe") } else { stem.to_owned() }
}

fn package_binary_file_name(ctx: &PackagingContext, package_name: &str) -> String {
    if ctx.target_platform == "win32" {
        format!("{package_name}.exe")
    } else {
        package_name.to_owned()
    }
}

fn temp_extension(path: &Path) -> String {
    match path.extension().and_then(OsStr::to_str) {
        Some(ext) if !ext.is_empty() => format!("{ext}.tmp"),
        _ => "tmp".into(),
    }
}

fn cargo_binary_output_path(ctx: &PackagingContext, package_name: &str) -> Result<PathBuf> {
    let file_name = package_binary_file_name(ctx, package_name);
    if ctx.os_name == env::consts::OS && normalize_host_arch(env::consts::ARCH) == ctx.target_arch {
        return Ok(ctx.project_root.join("target").join("release").join(file_name));
    }
    let target = rust_target_triple(ctx)?;
    Ok(ctx.project_root.join("target").join(target).join("release").join(file_name))
}

fn ensure_bundle_exists(ctx: &PackagingContext, bundle_root: &Path) -> Result<()> {
    if bundle_root.exists() {
        return Ok(());
    }
    if bundle_root == ctx.bundle_root {
        bundle_command(ctx)?;
        return Ok(());
    }
    bail!("bundle root not found: {}", bundle_root.display())
}

fn stage_windows_payload(
    bundle_root: &Path,
    stage_root: &Path,
    include_nsis: bool,
    include_service: bool,
) -> Result<()> {
    for binary_name in [
        "rtc-agent.exe",
        "rtc-agent-manager.exe",
        "rtc-agent-desktop.exe",
        "rtc-agent-installer.exe",
    ] {
        copy_file(
            &bundle_root.join("bin").join(binary_name),
            &stage_root.join("bin").join(binary_name),
        )?;
    }

    copy_file(
        &bundle_root.join("packaging").join("windows").join("agent.config.json"),
        &stage_root.join("packaging").join("windows").join("agent.config.json"),
    )?;
    if include_service {
        copy_file(
            &bundle_root
                .join("packaging")
                .join("windows")
                .join("RemoteTerminalCloudAgentService.xml"),
            &stage_root
                .join("service")
                .join("RemoteTerminalCloudAgentService.xml"),
        )?;
    }
    copy_tree(
        &bundle_root.join("packaging").join("windows").join("wix"),
        &stage_root.join("packaging").join("windows").join("wix"),
    )?;
    if include_nsis {
        copy_tree(
            &bundle_root.join("packaging").join("windows").join("nsis"),
            &stage_root.join("packaging").join("windows").join("nsis"),
        )?;
    }
    let version_file = bundle_root.join("VERSION");
    if version_file.exists() {
        copy_file(&version_file, &stage_root.join("VERSION"))?;
    }
    Ok(())
}

fn validate_windows_bundle_root(bundle_root: &Path, require_nsis: bool, require_wix: bool) -> Result<()> {
    let mut required = vec![
        bundle_root.join("bin").join("rtc-agent.exe"),
        bundle_root.join("bin").join("rtc-agent-manager.exe"),
        bundle_root.join("bin").join("rtc-agent-desktop.exe"),
        bundle_root.join("bin").join("rtc-agent-installer.exe"),
        bundle_root.join("packaging").join("windows").join("agent.config.json"),
        bundle_root
            .join("packaging")
            .join("windows")
            .join("RemoteTerminalCloudAgentService.xml"),
    ];
    if require_nsis {
        required.push(
            bundle_root
                .join("packaging")
                .join("windows")
                .join("nsis")
                .join("agent.nsi"),
        );
    }
    if require_wix {
        required.push(
            bundle_root
                .join("packaging")
                .join("windows")
                .join("wix")
                .join("RemoteTerminalCloudAgent.wxs"),
        );
    }
    validate_required_paths("bundle input", &required)
}

fn validate_windows_build_root(build_root: &Path, require_nsis: bool, require_wix: bool) -> Result<()> {
    let mut required = vec![
        build_root.join("bin").join("rtc-agent.exe"),
        build_root.join("bin").join("rtc-agent-manager.exe"),
        build_root.join("bin").join("rtc-agent-desktop.exe"),
        build_root.join("bin").join("rtc-agent-installer.exe"),
        build_root.join("packaging").join("windows").join("agent.config.json"),
    ];
    if require_nsis {
        required.push(
            build_root
                .join("packaging")
                .join("windows")
                .join("nsis")
                .join("agent.nsi"),
        );
    }
    if require_wix {
        required.push(
            build_root
                .join("packaging")
                .join("windows")
                .join("wix")
                .join("RemoteTerminalCloudAgent.wxs"),
        );
    }
    validate_required_paths("build input", &required)
}

fn validate_required_paths(kind: &str, required: &[PathBuf]) -> Result<()> {
    for path in required {
        if !path.exists() {
            bail!("required {kind} missing: {}", path.display());
        }
    }
    Ok(())
}

fn prepare_clean_directory(path: &Path, force: bool) -> Result<()> {
    if path.exists() {
        if !force {
            bail!(
                "path already exists: {} (re-run with --force to replace it)",
                path.display()
            );
        }
        fs::remove_dir_all(path).with_context(|| format!("remove {}", path.display()))?;
    }
    fs::create_dir_all(path).with_context(|| format!("create {}", path.display()))
}

fn load_version_from_build_root(build_root: &Path) -> Option<String> {
    fs::read_to_string(build_root.join("VERSION"))
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn find_nsis_executable(override_path: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = override_path {
        return Ok(resolve_path(path));
    }
    if let Ok(path) = which("makensis.exe").or_else(|_| which("makensis")) {
        return Ok(path);
    }
    if let Some(path) = first_existing_file(&nsis_executable_candidates()) {
        return Ok(path);
    }
    bail!(
        "makensis.exe not found. Checked PATH plus standard, WinGet, Chocolatey, Scoop, and NSIS_HOME/NSIS_ROOT locations. Install NSIS, reopen the shell, or pass --nsis-exe"
    )
}

fn find_executable(override_path: Option<&Path>, candidates: &[&str]) -> Result<PathBuf> {
    if let Some(path) = override_path {
        return Ok(resolve_path(path));
    }
    for candidate in candidates {
        if let Ok(path) = which(candidate) {
            return Ok(path);
        }
    }
    bail!("executable not found")
}

fn nsis_executable_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut add = |value: Option<String>, parts: &[&str]| {
        let Some(value) = value.map(|item| item.trim().to_owned()).filter(|item| !item.is_empty())
        else {
            return;
        };
        let mut candidate = PathBuf::from(value);
        for part in parts {
            candidate.push(part);
        }
        candidates.push(candidate);
    };

    let program_files = env::var("ProgramFiles").ok();
    let program_files_x86 = env::var("ProgramFiles(x86)").ok();
    let local_app_data = env::var("LOCALAPPDATA").ok();
    let chocolatey_install = env::var("ChocolateyInstall").ok();
    let user_profile = env::var("USERPROFILE").ok();
    let scoop_root = env::var("SCOOP").ok().or_else(|| user_profile.map(|value| format!("{value}\\scoop")));
    let nsis_home = env::var("NSIS_HOME").ok();
    let nsis_root = env::var("NSIS_ROOT").ok();

    add(program_files_x86.clone(), &["NSIS", "makensis.exe"]);
    add(program_files.clone(), &["NSIS", "makensis.exe"]);
    add(program_files_x86.clone(), &["Nullsoft Scriptable Install System", "makensis.exe"]);
    add(program_files.clone(), &["Nullsoft Scriptable Install System", "makensis.exe"]);
    add(local_app_data.clone(), &["Programs", "NSIS", "makensis.exe"]);
    add(local_app_data.clone(), &["NSIS", "makensis.exe"]);
    add(chocolatey_install.clone(), &["bin", "makensis.exe"]);
    add(chocolatey_install.clone(), &["lib", "nsis", "tools", "makensis.exe"]);
    add(chocolatey_install.clone(), &["lib", "nsis.portable", "tools", "makensis.exe"]);
    add(scoop_root.clone(), &["shims", "makensis.exe"]);
    add(scoop_root.clone(), &["apps", "nsis", "current", "makensis.exe"]);
    add(nsis_home, &["makensis.exe"]);
    add(nsis_root, &["makensis.exe"]);

    for pattern in [
        local_app_data
            .as_ref()
            .map(|base| format!(r"{base}\Microsoft\WinGet\Packages\*NSIS*\makensis.exe")),
        local_app_data
            .as_ref()
            .map(|base| format!(r"{base}\Microsoft\WinGet\Packages\*NSIS*\*\makensis.exe")),
        local_app_data
            .as_ref()
            .map(|base| format!(r"{base}\Microsoft\WinGet\Packages\*NSIS*\*\*\makensis.exe")),
        local_app_data
            .as_ref()
            .map(|base| format!(r"{base}\Microsoft\WinGet\Packages\*Nullsoft*\makensis.exe")),
        local_app_data
            .as_ref()
            .map(|base| format!(r"{base}\Microsoft\WinGet\Packages\*Nullsoft*\*\makensis.exe")),
        local_app_data
            .as_ref()
            .map(|base| format!(r"{base}\Microsoft\WinGet\Packages\*Nullsoft*\*\*\makensis.exe")),
        program_files_x86.as_ref().map(|base| format!(r"{base}\*NSIS*\makensis.exe")),
        program_files.as_ref().map(|base| format!(r"{base}\*NSIS*\makensis.exe")),
        chocolatey_install.as_ref().map(|base| format!(r"{base}\lib\nsis*\tools\makensis.exe")),
        chocolatey_install
            .as_ref()
            .map(|base| format!(r"{base}\lib\nsis*\tools\*\makensis.exe")),
        scoop_root.as_ref().map(|base| format!(r"{base}\apps\nsis\*\makensis.exe")),
    ]
    .into_iter()
    .flatten()
    {
        if let Ok(paths) = glob_paths(&pattern) {
            candidates.extend(paths);
        }
    }

    dedupe_paths(candidates)
}

fn first_existing_file(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|path| path.is_file()).cloned()
}

fn detect_wix_version(path: &Path) -> Result<(Option<String>, Option<i32>)> {
    let output = Command::new(path)
        .arg("--version")
        .stderr(Stdio::inherit())
        .output()
        .with_context(|| format!("run {} --version", path.display()))?;
    if !output.status.success() {
        bail!("failed to detect WiX version");
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let version = text
        .split_whitespace()
        .find(|token| token.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
        .map(str::to_owned);
    let major = version
        .as_ref()
        .and_then(|value| value.split('.').next())
        .and_then(|value| value.parse::<i32>().ok());
    Ok((version, major))
}

fn run_streaming_command(cwd: Option<&Path>, command: &Path, args: &[String]) -> Result<()> {
    let mut cmd = Command::new(command);
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    cmd.args(args);
    run_command(&mut cmd, &format!("command failed: {} {}", command.display(), args.join(" ")))
}

fn download_winsw_executable(target_exe: &Path, version: &str, force: bool) -> Result<()> {
    let winsw_version = if version.trim().is_empty() { DEFAULT_WINSW_VERSION } else { version };
    if target_exe.exists() && !force {
        return Ok(());
    }
    if let Some(parent) = target_exe.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }

    let url = format!("https://github.com/winsw/winsw/releases/download/{winsw_version}/WinSW-x64.exe");
    let response = Client::new().get(&url).send().with_context(|| format!("download {url}"))?;
    if !response.status().is_success() {
        bail!("failed to download WinSW from {url}: {}", response.status());
    }
    let mut bytes = io::Cursor::new(response.bytes()?.to_vec());
    let temp = target_exe.with_extension("download");
    let mut file = File::create(&temp).with_context(|| format!("create {}", temp.display()))?;
    io::copy(&mut bytes, &mut file).with_context(|| format!("write {}", temp.display()))?;
    file.flush().ok();
    replace_file(&temp, target_exe)?;
    Ok(())
}

fn create_archive(ctx: &PackagingContext) -> Result<()> {
    fs::create_dir_all(&ctx.platform_out_root)
        .with_context(|| format!("create {}", ctx.platform_out_root.display()))?;
    if ctx.target_platform == "win32" {
        zip_directory(&ctx.stage_root, &ctx.platform_out_root.join(archive_file_name(ctx)))
    } else {
        tar_gz_directory(
            &ctx.platform_out_root,
            ctx.stage_root
                .file_name()
                .and_then(OsStr::to_str)
                .ok_or_else(|| anyhow!("invalid stage root name"))?,
            &ctx.platform_out_root.join(archive_file_name(ctx)),
        )
    }
}

fn archive_file_name(ctx: &PackagingContext) -> String {
    if ctx.target_platform == "win32" {
        format!("{}.zip", ctx.archive_base_name)
    } else {
        format!("{}.tar.gz", ctx.archive_base_name)
    }
}

fn native_installer_file_name(ctx: &PackagingContext) -> Option<String> {
    match ctx.target_platform.as_str() {
        "linux" => Some(format!("{}.deb", ctx.archive_base_name)),
        "darwin" => Some(format!("{}.pkg", ctx.archive_base_name)),
        _ => None,
    }
}

fn start_command(ctx: &PackagingContext) -> String {
    if ctx.target_platform == "win32" {
        format!(r".\bin\{}", binary_file_name(ctx, "rtc-agent"))
    } else {
        format!("./bin/{}", binary_file_name(ctx, "rtc-agent"))
    }
}

fn build_native_installer(ctx: &PackagingContext) -> Result<()> {
    match ctx.target_platform.as_str() {
        "linux" => {
            let mut cmd = Command::new("bash");
            cmd.current_dir(&ctx.project_root)
                .arg(
                    ctx.stage_root
                        .join("packaging")
                        .join("linux")
                        .join("build-deb.sh"),
                )
                .arg(&ctx.stage_root)
                .arg(ctx.platform_out_root.join(format!("{}.deb", ctx.archive_base_name)))
                .arg(&ctx.version)
                .arg(&ctx.target_arch);
            run_command(&mut cmd, "Linux .deb build failed")
        }
        "darwin" => {
            let mut cmd = Command::new("bash");
            cmd.current_dir(&ctx.project_root)
                .arg(
                    ctx.stage_root
                        .join("packaging")
                        .join("macos")
                        .join("build-pkg.sh"),
                )
                .arg(&ctx.stage_root)
                .arg(ctx.platform_out_root.join(format!("{}.pkg", ctx.archive_base_name)))
                .arg(&ctx.version)
                .arg(&ctx.target_arch);
            run_command(&mut cmd, "macOS .pkg build failed")
        }
        _ => Ok(()),
    }
}

fn read_version(project_root: &Path) -> Result<String> {
    let version = fs::read_to_string(project_root.join("VERSION"))
        .with_context(|| format!("read {}", project_root.join("VERSION").display()))?;
    let version = version.trim().to_owned();
    if version.is_empty() {
        bail!("VERSION is empty");
    }
    Ok(version)
}

fn now_rfc3339() -> Result<String> {
    let timestamp = OffsetDateTime::from(SystemTime::now());
    timestamp.format(&Rfc3339).context("format timestamp")
}

fn write_json(path: &Path, value: serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let payload = serde_json::to_string_pretty(&value)?;
    fs::write(path, payload).with_context(|| format!("write {}", path.display()))
}

fn zip_directory(src_dir: &Path, output_path: &Path) -> Result<()> {
    let file = File::create(output_path).with_context(|| format!("create {}", output_path.display()))?;
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    for path in walk_files(src_dir)? {
        let relative = path.strip_prefix(src_dir).with_context(|| {
            format!("strip prefix {} from {}", src_dir.display(), path.display())
        })?;
        writer.start_file(relative.to_string_lossy().replace('\\', "/"), options)?;
        let mut source = File::open(&path).with_context(|| format!("open {}", path.display()))?;
        io::copy(&mut source, &mut writer).with_context(|| format!("zip {}", path.display()))?;
    }
    writer.finish()?;
    Ok(())
}

fn tar_gz_directory(parent_dir: &Path, folder_name: &str, output_path: &Path) -> Result<()> {
    let file = File::create(output_path).with_context(|| format!("create {}", output_path.display()))?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = TarBuilder::new(encoder);
    builder.follow_symlinks(false);
    builder.append_dir_all(folder_name, parent_dir.join(folder_name))?;
    builder.finish()?;
    Ok(())
}

fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(src).with_context(|| format!("read {}", src.display()))? {
        let entry = entry?;
        let entry_path = entry.path();
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry_path, &target)?;
        } else {
            copy_file(&entry_path, &target)?;
        }
    }
    Ok(())
}

fn copy_file(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    fs::copy(src, dst)
        .with_context(|| format!("copy {} -> {}", src.display(), dst.display()))?;
    let metadata = fs::metadata(src).with_context(|| format!("metadata {}", src.display()))?;
    fs::set_permissions(dst, metadata.permissions())
        .with_context(|| format!("chmod {}", dst.display()))?;
    Ok(())
}

fn replace_file(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    if dst.exists() {
        fs::remove_file(dst).with_context(|| {
            format!(
                "failed to replace {} (the file may still be in use by rtc-agent or rtc-agent-manager)",
                dst.display()
            )
        })?;
    }
    fs::rename(src, dst)
        .with_context(|| format!("failed to move {} into place", dst.display()))?;
    Ok(())
}

fn run_command(cmd: &mut Command, context_message: &str) -> Result<()> {
    cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    let status = cmd.status().with_context(|| context_message.to_owned())?;
    if !status.success() {
        bail!("{context_message}: exited with status {status}");
    }
    Ok(())
}

fn repo_root() -> Result<PathBuf> {
    let mut dir = env::current_dir().context("read current dir")?;
    loop {
        if dir.join("Cargo.toml").is_file() && dir.join("apps").is_dir() && dir.join("crates").is_dir() {
            return Ok(dir);
        }
        if !dir.pop() {
            bail!("failed to locate workspace root");
        }
    }
}

fn env_or(name: &str, fallback: &str) -> String {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}

fn normalize_target_platform(value: String) -> String {
    match value.as_str() {
        "windows" => "win32".into(),
        "win32" | "linux" | "darwin" => value,
        _ => env::consts::OS.to_owned(),
    }
}

fn normalize_target_arch(value: String) -> String {
    match value.as_str() {
        "amd64" => "x64".into(),
        "x64" | "arm64" => value,
        _ => normalize_host_arch(&value),
    }
}

fn normalize_host_arch(value: &str) -> String {
    match value {
        "x86_64" | "amd64" => "x64".into(),
        "aarch64" | "arm64" => "arm64".into(),
        other => other.to_owned(),
    }
}

fn rust_target_triple(ctx: &PackagingContext) -> Result<String> {
    match (ctx.target_platform.as_str(), ctx.target_arch.as_str()) {
        ("win32", "x64") => Ok("x86_64-pc-windows-msvc".into()),
        ("win32", "arm64") => Ok("aarch64-pc-windows-msvc".into()),
        ("linux", "x64") => Ok("x86_64-unknown-linux-gnu".into()),
        ("linux", "arm64") => Ok("aarch64-unknown-linux-gnu".into()),
        ("darwin", "x64") => Ok("x86_64-apple-darwin".into()),
        ("darwin", "arm64") => Ok("aarch64-apple-darwin".into()),
        _ => bail!(
            "unsupported target combination for Rust packaging: {}/{}",
            ctx.target_platform,
            ctx.target_arch
        ),
    }
}

fn slash_join<const N: usize>(parts: [&str; N]) -> String {
    parts.join("/")
}

fn resolve_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = BTreeMap::<String, PathBuf>::new();
    for path in paths {
        let key = path.to_string_lossy().to_string();
        seen.entry(key).or_insert(path);
    }
    seen.into_values().collect()
}

fn glob_paths(pattern: &str) -> Result<Vec<PathBuf>> {
    let mut cmd = Command::new("powershell");
    cmd.arg("-NoProfile").arg("-Command").arg(format!(
        "Get-ChildItem -Path '{}' -ErrorAction SilentlyContinue | ForEach-Object {{ $_.FullName }}",
        pattern.replace('\'', "''")
    ));
    let output = cmd.output().context("resolve glob pattern")?;
    if !output.status.success() {
        return Ok(Vec::new());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect())
}

fn walk_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !root.exists() {
        return Ok(files);
    }
    for entry in fs::read_dir(root).with_context(|| format!("read {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            files.extend(walk_files(&path)?);
        } else {
            files.push(path);
        }
    }
    Ok(files)
}

fn xtask_command_line(command: &str, args: &[(&str, Option<&Path>)]) -> String {
    let mut parts = vec!["cargo".to_owned(), "xtask".to_owned(), command.to_owned()];
    for (flag, value) in args {
        parts.push((*flag).to_owned());
        if let Some(value) = value {
            let rendered = value.display().to_string();
            if rendered.contains(' ') {
                parts.push(format!("\"{rendered}\""));
            } else {
                parts.push(rendered);
            }
        }
    }
    parts.join(" ")
}

fn success(command: &str, message: &str, details: BTreeMap<String, String>) -> PackagingActionResult {
    PackagingActionResult {
        command: command.to_owned(),
        ok: true,
        message: message.to_owned(),
        details,
    }
}
