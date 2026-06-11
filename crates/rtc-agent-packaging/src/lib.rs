use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

use anyhow::{Context, Result, anyhow, bail};
use flate2::Compression;
use flate2::write::GzEncoder;
use rtc_agent_config::RELEASE_SERVER_BASE_URL;
use serde::Serialize;
use tar::Builder as TarBuilder;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use zip::CompressionMethod;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

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
    Package,
    WindowsDesktopBundle {
        output_dir: Option<PathBuf>,
        bundles: String,
        target: Option<String>,
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
        PackagingCommand::Package => package_command(&ctx),
        PackagingCommand::WindowsDesktopBundle { output_dir, bundles, target } => {
            windows_desktop_bundle_command(&ctx, output_dir.as_deref(), &bundles, target.as_deref())
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

fn package_command(ctx: &PackagingContext) -> Result<PackagingActionResult> {
    if ctx.target_platform == "win32" {
        let output_dir = ctx
            .project_root
            .join("release")
            .join("artifacts")
            .join("windows-installers")
            .join("tauri");
        let result = windows_desktop_bundle_command(ctx, Some(&output_dir), "nsis", None)?;
        let mut details = result.details;
        details.insert("mode".into(), "windows-desktop-bundle".into());
        return Ok(success(
            "package",
            "Windows desktop package built.",
            details,
        ));
    }

    let result = artifact_command(ctx)?;
    let mut details = result.details;
    details.insert("mode".into(), "artifact".into());
    Ok(success("package", "Platform package built.", details))
}

fn windows_desktop_bundle_command(
    ctx: &PackagingContext,
    output_dir: Option<&Path>,
    bundles: &str,
    target: Option<&str>,
) -> Result<PackagingActionResult> {
    if ctx.target_platform != "win32" {
        bail!("windows-desktop-bundle is only supported when RTC_TARGET_PLATFORM=win32");
    }

    let desktop_dir = ctx.project_root.join("apps").join("rtc-agent-desktop");
    let output_dir = output_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| ctx.project_root.join("release").join("artifacts").join("windows-installers").join("tauri"));
    fs::create_dir_all(&output_dir).with_context(|| format!("create {}", output_dir.display()))?;

    let stage_dir = ctx.project_root.join("target").join("xtask").join("tauri-desktop-bundle");
    if stage_dir.exists() {
        fs::remove_dir_all(&stage_dir).with_context(|| format!("remove {}", stage_dir.display()))?;
    }
    fs::create_dir_all(&stage_dir).with_context(|| format!("create {}", stage_dir.display()))?;

    let target_triple = target
        .map(|value| value.to_owned())
        .unwrap_or_else(|| rust_target_triple(ctx).unwrap_or_else(|_| "x86_64-pc-windows-msvc".to_owned()));
    let sidecar_dir = stage_dir.join("deps");
    fs::create_dir_all(&sidecar_dir).with_context(|| format!("create {}", sidecar_dir.display()))?;

    build_cli_binary(
        ctx,
        "rtc-agentd",
        &sidecar_dir.join(format!("rtc-agentd-{target_triple}.exe")),
        false,
    )?;
    build_desktop_binary(ctx, &stage_dir)?;

    let mut npm = Command::new(node_package_manager_command());
    npm.current_dir(&desktop_dir).arg("run").arg("build");
    run_command(&mut npm, "desktop frontend build failed")?;

    let tauri_config_patch = serde_json::json!({
        "bundle": {
            "externalBin": [
                sidecar_dir.join("rtc-agentd").display().to_string()
            ],
            "windows": {
                "nsis": {
                    "template": desktop_dir
                        .join("src-tauri")
                        .join("nsis")
                        .join("installer.nsi")
                        .display()
                        .to_string()
                }
            }
        }
    })
    .to_string();

    let mut tauri = Command::new(node_package_manager_command());
    tauri
        .current_dir(&desktop_dir.join("src-tauri"))
        .arg("run")
        .arg("tauri")
        .arg("--")
        .arg("build")
        .arg("--bundles")
        .arg(bundles)
        .env("TAURI_CONFIG", tauri_config_patch)
        .env("RTC_AGENT_SERVER_BASE_URL", RELEASE_SERVER_BASE_URL)
        .env("RTC_AGENT_BUNDLE_OUTPUT_DIR", &output_dir);
    if let Some(target) = target.filter(|value| !value.trim().is_empty()) {
        tauri.arg("--target").arg(target);
    }
    run_command(&mut tauri, "tauri desktop bundle failed")?;

    let tauri_bundle_root = ctx.project_root.join("target").join("release").join("bundle");
    if tauri_bundle_root.exists() {
        copy_tree(&tauri_bundle_root, &output_dir)?;
    }

    let mut details = BTreeMap::new();
    details.insert("outputDir".into(), output_dir.display().to_string());
    details.insert("bundles".into(), bundles.to_owned());
    Ok(success(
        "windows-desktop-bundle",
        "Tauri desktop bundle built.",
        details,
    ))
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
            format!("failed to replace {} (the file may still be in use by rtc-agent or rtc-agent-desktop)", dst.display())
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

fn success(command: &str, message: &str, details: BTreeMap<String, String>) -> PackagingActionResult {
    PackagingActionResult {
        command: command.to_owned(),
        ok: true,
        message: message.to_owned(),
        details,
    }
}
