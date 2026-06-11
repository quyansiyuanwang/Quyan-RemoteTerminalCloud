package main

import (
	"archive/tar"
	"archive/zip"
	"compress/gzip"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"time"

	"github.com/remote-terminal-cloud/agent/internal/buildinfo"
)

const releaseServerBaseURL = "https://api.qysyw.cn"

type releaseConfig struct {
	projectRoot     string
	version         string
	targetPlatform  string
	targetArch      string
	goos            string
	goarch          string
	agentBinaryName string
	managerBinaryName string
	installerBinaryName string
	releaseRoot     string
	bundleRoot      string
	platformOutRoot string
	stageRoot       string
	archiveBaseName string
}

func main() {
	if len(os.Args) < 2 {
		exitWith("usage: go run ./cmd/rtc-release <build|bundle|artifact>")
	}

	root, err := repoRoot()
	if err != nil {
		exitWith(err.Error())
	}

	cfg := newReleaseConfig(root)

	switch os.Args[1] {
	case "build":
		err = buildBinary(cfg)
	case "bundle":
		err = buildBundle(cfg)
	case "artifact":
		err = buildArtifact(cfg)
	default:
		err = fmt.Errorf("unknown command: %s", os.Args[1])
	}

	if err != nil {
		exitWith(err.Error())
	}
}

func newReleaseConfig(root string) releaseConfig {
	version := buildinfo.LoadVersionFromFile()

	targetPlatform := envOr("RTC_TARGET_PLATFORM", runtime.GOOS)
	switch targetPlatform {
	case "windows":
		targetPlatform = "win32"
	case "darwin", "linux", "win32":
	default:
		targetPlatform = runtime.GOOS
	}

	targetArch := envOr("RTC_TARGET_ARCH", runtime.GOARCH)
	switch targetArch {
	case "amd64":
		targetArch = "x64"
	case "arm64", "x64":
	default:
		targetArch = runtime.GOARCH
	}

	goos := targetPlatform
	if targetPlatform == "win32" {
		goos = "windows"
	}

	goarch := targetArch
	if targetArch == "x64" {
		goarch = "amd64"
	}

	agentBinaryName := "rtc-agent"
	managerBinaryName := "rtc-agent-manager"
	installerBinaryName := "rtc-agent-installer"
	if targetPlatform == "win32" {
		agentBinaryName += ".exe"
		managerBinaryName += ".exe"
		installerBinaryName += ".exe"
	}

	releaseRoot := filepath.Join(root, "release")
	bundleRoot := filepath.Join(releaseRoot, fmt.Sprintf("remote-terminal-cloud-agent-%s", version))
	platformOutRoot := filepath.Join(releaseRoot, "artifacts", fmt.Sprintf("%s-%s", targetPlatform, targetArch))
	stageRoot := filepath.Join(platformOutRoot, fmt.Sprintf("remote-terminal-cloud-agent-%s", version))
	archiveBaseName := fmt.Sprintf("remote-terminal-cloud-agent-%s-%s-%s", version, targetPlatform, targetArch)

	return releaseConfig{
		projectRoot:     root,
		version:         version,
		targetPlatform:  targetPlatform,
		targetArch:      targetArch,
		goos:            goos,
		goarch:          goarch,
		agentBinaryName: agentBinaryName,
		managerBinaryName: managerBinaryName,
		installerBinaryName: installerBinaryName,
		releaseRoot:     releaseRoot,
		bundleRoot:      bundleRoot,
		platformOutRoot: platformOutRoot,
		stageRoot:       stageRoot,
		archiveBaseName: archiveBaseName,
	}
}

func buildBinary(cfg releaseConfig) error {
	outputDir := filepath.Join(cfg.projectRoot, "build", "bin", fmt.Sprintf("%s-%s", cfg.targetPlatform, cfg.targetArch))
	if err := os.MkdirAll(outputDir, 0o755); err != nil {
		return err
	}

	ldflags := fmt.Sprintf("-X github.com/remote-terminal-cloud/agent/internal/buildinfo.ServerBaseURL=%s", releaseServerBaseURL)
	for _, target := range []struct {
		outputPath string
		packagePath string
		guiBinary   bool
	}{
		{outputPath: filepath.Join(outputDir, cfg.agentBinaryName), packagePath: "./cmd/rtc-agent"},
		{outputPath: filepath.Join(outputDir, cfg.managerBinaryName), packagePath: "./cmd/rtc-agent-manager", guiBinary: cfg.goos == "windows"},
		{outputPath: filepath.Join(outputDir, cfg.installerBinaryName), packagePath: "./cmd/rtc-agent-installer"},
	} {
		targetLdflags := ldflags
		if target.guiBinary {
			targetLdflags += " -H=windowsgui"
		}
		args := []string{"build", "-ldflags", targetLdflags, "-o", target.outputPath, target.packagePath}
		cmd := exec.Command("go", args...)
		cmd.Dir = cfg.projectRoot
		cmd.Env = append(os.Environ(),
			"GOOS="+cfg.goos,
			"GOARCH="+cfg.goarch,
			"CGO_ENABLED=0",
		)
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		if err := cmd.Run(); err != nil {
			return err
		}
	}
	return nil
}

func buildBundle(cfg releaseConfig) error {
	if err := os.RemoveAll(cfg.releaseRoot); err != nil {
		return err
	}
	if err := os.MkdirAll(cfg.releaseRoot, 0o755); err != nil {
		return err
	}

	if err := buildBinary(cfg); err != nil {
		return err
	}

	if err := os.MkdirAll(filepath.Join(cfg.bundleRoot, "bin"), 0o755); err != nil {
		return err
	}

	sourceAgentBinary := filepath.Join(cfg.projectRoot, "build", "bin", fmt.Sprintf("%s-%s", cfg.targetPlatform, cfg.targetArch), cfg.agentBinaryName)
	if err := copyFile(sourceAgentBinary, filepath.Join(cfg.bundleRoot, "bin", cfg.agentBinaryName)); err != nil {
		return err
	}
	sourceManagerBinary := filepath.Join(cfg.projectRoot, "build", "bin", fmt.Sprintf("%s-%s", cfg.targetPlatform, cfg.targetArch), cfg.managerBinaryName)
	if err := copyFile(sourceManagerBinary, filepath.Join(cfg.bundleRoot, "bin", cfg.managerBinaryName)); err != nil {
		return err
	}
	sourceInstallerBinary := filepath.Join(cfg.projectRoot, "build", "bin", fmt.Sprintf("%s-%s", cfg.targetPlatform, cfg.targetArch), cfg.installerBinaryName)
	if err := copyFile(sourceInstallerBinary, filepath.Join(cfg.bundleRoot, "bin", cfg.installerBinaryName)); err != nil {
		return err
	}

	for _, dir := range []string{"cmd", "internal", "packaging", "docs"} {
		if err := copyTree(filepath.Join(cfg.projectRoot, dir), filepath.Join(cfg.bundleRoot, dir)); err != nil {
			return err
		}
	}

	if err := copyFile(filepath.Join(cfg.projectRoot, "VERSION"), filepath.Join(cfg.bundleRoot, "VERSION")); err != nil {
		return err
	}

	for _, platformDir := range []string{"windows", "linux", "macos"} {
		if err := os.MkdirAll(filepath.Join(cfg.bundleRoot, "artifacts", platformDir), 0o755); err != nil {
			return err
		}
	}

	return writeJSON(filepath.Join(cfg.bundleRoot, "bundle.json"), map[string]any{
		"name":    "rtc-agent",
		"version": cfg.version,
		"binary":  filepath.ToSlash(filepath.Join("bin", cfg.agentBinaryName)),
		"managerBinary": filepath.ToSlash(filepath.Join("bin", cfg.managerBinaryName)),
		"installerBinary": filepath.ToSlash(filepath.Join("bin", cfg.installerBinaryName)),
	})
}

func buildArtifact(cfg releaseConfig) error {
	if _, err := os.Stat(cfg.bundleRoot); errors.Is(err, os.ErrNotExist) {
		if err := buildBundle(cfg); err != nil {
			return err
		}
	}

	if err := os.RemoveAll(cfg.platformOutRoot); err != nil {
		return err
	}
	if err := os.MkdirAll(cfg.stageRoot, 0o755); err != nil {
		return err
	}

	if err := copyTree(cfg.bundleRoot, cfg.stageRoot); err != nil {
		return err
	}

	if err := writeJSON(filepath.Join(cfg.stageRoot, "ARTIFACT-INFO.json"), map[string]any{
		"generatedAt":         time.Now().UTC().Format(time.RFC3339),
		"version":             cfg.version,
		"targetPlatform":      cfg.targetPlatform,
		"targetArch":          cfg.targetArch,
		"archiveFile":         archiveFileName(cfg),
		"nativeInstallerFile": nativeInstallerFileName(cfg),
		"binaryPath":          filepath.ToSlash(filepath.Join("bin", cfg.agentBinaryName)),
		"managerBinaryPath":   filepath.ToSlash(filepath.Join("bin", cfg.managerBinaryName)),
		"installerBinaryPath": filepath.ToSlash(filepath.Join("bin", cfg.installerBinaryName)),
		"startCommand":        startCommand(cfg),
	}); err != nil {
		return err
	}

	readme := strings.Join([]string{
		"Remote Terminal Cloud Agent platform artifact",
		"",
		"Version: " + cfg.version,
		"Platform: " + cfg.targetPlatform,
		"Architecture: " + cfg.targetArch,
		"Server base URL: " + releaseServerBaseURL,
		"",
		"This artifact contains:",
		"- bin/ Go agent and manager binaries",
		"- packaging/ platform service installation templates",
	}, "\n")
	if err := os.WriteFile(filepath.Join(cfg.stageRoot, "README.txt"), []byte(readme), 0o644); err != nil {
		return err
	}

	if err := createArchive(cfg); err != nil {
		return err
	}

	return buildNativeInstaller(cfg)
}

func buildNativeInstaller(cfg releaseConfig) error {
	switch cfg.targetPlatform {
	case "linux":
		return runCommand(cfg.projectRoot, "bash",
			filepath.Join(cfg.stageRoot, "packaging", "linux", "build-deb.sh"),
			cfg.stageRoot,
			filepath.Join(cfg.platformOutRoot, cfg.archiveBaseName+".deb"),
			cfg.version,
			cfg.targetArch,
		)
	case "darwin":
		return runCommand(cfg.projectRoot, "bash",
			filepath.Join(cfg.stageRoot, "packaging", "macos", "build-pkg.sh"),
			cfg.stageRoot,
			filepath.Join(cfg.platformOutRoot, cfg.archiveBaseName+".pkg"),
			cfg.version,
			cfg.targetArch,
		)
	default:
		return nil
	}
}

func createArchive(cfg releaseConfig) error {
	if err := os.MkdirAll(cfg.platformOutRoot, 0o755); err != nil {
		return err
	}

	if cfg.targetPlatform == "win32" {
		return zipDirectory(cfg.stageRoot, filepath.Join(cfg.platformOutRoot, cfg.archiveBaseName+".zip"))
	}

	return tarGzDirectory(cfg.platformOutRoot, filepath.Base(cfg.stageRoot), filepath.Join(cfg.platformOutRoot, cfg.archiveBaseName+".tar.gz"))
}

func archiveFileName(cfg releaseConfig) string {
	if cfg.targetPlatform == "win32" {
		return cfg.archiveBaseName + ".zip"
	}
	return cfg.archiveBaseName + ".tar.gz"
}

func nativeInstallerFileName(cfg releaseConfig) any {
	switch cfg.targetPlatform {
	case "linux":
		return cfg.archiveBaseName + ".deb"
	case "darwin":
		return cfg.archiveBaseName + ".pkg"
	default:
		return nil
	}
}

func startCommand(cfg releaseConfig) string {
	if cfg.targetPlatform == "win32" {
		return ".\\bin\\" + cfg.agentBinaryName
	}
	return "./bin/" + cfg.agentBinaryName
}

func copyTree(src string, dst string) error {
	return filepath.WalkDir(src, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}

		rel, err := filepath.Rel(src, path)
		if err != nil {
			return err
		}
		target := filepath.Join(dst, rel)

		if d.IsDir() {
			return os.MkdirAll(target, 0o755)
		}
		return copyFile(path, target)
	})
}

func copyFile(src string, dst string) error {
	if err := os.MkdirAll(filepath.Dir(dst), 0o755); err != nil {
		return err
	}

	in, err := os.Open(src)
	if err != nil {
		return err
	}
	defer in.Close()

	info, err := in.Stat()
	if err != nil {
		return err
	}

	out, err := os.OpenFile(dst, os.O_CREATE|os.O_TRUNC|os.O_WRONLY, info.Mode())
	if err != nil {
		return err
	}
	defer out.Close()

	_, err = io.Copy(out, in)
	return err
}

func writeJSON(path string, value any) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	data, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(path, data, 0o644)
}

func zipDirectory(srcDir string, outputPath string) error {
	file, err := os.Create(outputPath)
	if err != nil {
		return err
	}
	defer file.Close()

	writer := zip.NewWriter(file)
	defer writer.Close()

	return filepath.Walk(srcDir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		if info.IsDir() {
			return nil
		}

		rel, err := filepath.Rel(srcDir, path)
		if err != nil {
			return err
		}

		entry, err := writer.Create(filepath.ToSlash(rel))
		if err != nil {
			return err
		}

		source, err := os.Open(path)
		if err != nil {
			return err
		}
		defer source.Close()

		_, err = io.Copy(entry, source)
		return err
	})
}

func tarGzDirectory(parentDir string, folderName string, outputPath string) error {
	file, err := os.Create(outputPath)
	if err != nil {
		return err
	}
	defer file.Close()

	gzipWriter := gzip.NewWriter(file)
	defer gzipWriter.Close()

	tarWriter := tar.NewWriter(gzipWriter)
	defer tarWriter.Close()

	root := filepath.Join(parentDir, folderName)
	return filepath.Walk(root, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}

		rel, err := filepath.Rel(parentDir, path)
		if err != nil {
			return err
		}

		header, err := tar.FileInfoHeader(info, "")
		if err != nil {
			return err
		}
		header.Name = filepath.ToSlash(rel)

		if err := tarWriter.WriteHeader(header); err != nil {
			return err
		}
		if info.IsDir() {
			return nil
		}

		source, err := os.Open(path)
		if err != nil {
			return err
		}
		defer source.Close()

		_, err = io.Copy(tarWriter, source)
		return err
	})
}

func runCommand(cwd string, command string, args ...string) error {
	cmd := exec.Command(command, args...)
	cmd.Dir = cwd
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	return cmd.Run()
}

func repoRoot() (string, error) {
	dir, err := os.Getwd()
	if err != nil {
		return "", err
	}

	for {
		if _, statErr := os.Stat(filepath.Join(dir, "go.mod")); statErr == nil {
			return dir, nil
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			return "", os.ErrNotExist
		}
		dir = parent
	}
}

func envOr(name string, fallback string) string {
	value := strings.TrimSpace(os.Getenv(name))
	if value == "" {
		return fallback
	}
	return value
}

func exitWith(message string) {
	fmt.Fprintln(os.Stderr, message)
	os.Exit(1)
}
