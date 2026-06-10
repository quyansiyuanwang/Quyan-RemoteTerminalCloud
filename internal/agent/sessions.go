package agent

import (
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"strings"
	"sync"

	gopty "github.com/aymanbagabas/go-pty"

	"github.com/remote-terminal-cloud/agent/internal/protocol"
)

type ShellSessionCallbacks struct {
	OnReady  func()
	OnOutput func(stream string, data string)
	OnExit   func(exitCode *int)
	OnError  func(message string)
}

type BrowseDirectoryResult struct {
	CurrentPath string
	ParentPath  string
	Items       []protocol.DirectoryEntry
}

type shellSession struct {
	pty    gopty.Pty
	cmd    *gopty.Cmd
	cancel context.CancelFunc
	close  sync.Once
	closed chan struct{}
}

type ShellSessionManager struct {
	defaultShellType protocol.ShellType
	mu               sync.Mutex
	sessions         map[string]*shellSession
}

func NewShellSessionManager(defaultShellType protocol.ShellType) *ShellSessionManager {
	return &ShellSessionManager{
		defaultShellType: defaultShellType,
		sessions:         make(map[string]*shellSession),
	}
}

func (m *ShellSessionManager) StartSession(sessionID string, shellType protocol.ShellType, callbacks ShellSessionCallbacks, workingDirectory string) {
	m.mu.Lock()
	if _, exists := m.sessions[sessionID]; exists {
		m.mu.Unlock()
		callbacks.OnError("Session already exists.")
		return
	}
	m.mu.Unlock()

	launch, err := resolveShellLaunch(shellType, m.defaultShellType)
	if err != nil {
		callbacks.OnError(err.Error())
		return
	}
	fmt.Printf("[remote-terminal-cloud-agent] starting session %s shell=%s executable=%s cwd=%s\n", sessionID, launch.shellType, launch.executable, strings.TrimSpace(workingDirectory))

	cwd, warning := resolveWorkingDirectory(workingDirectory)
	ptyInstance, err := gopty.New()
	if err != nil {
		callbacks.OnError(err.Error())
		return
	}

	if err := ptyInstance.Resize(120, 30); err != nil {
		_ = ptyInstance.Close()
		callbacks.OnError(err.Error())
		return
	}

	ctx, cancel := context.WithCancel(context.Background())
	cmd := ptyInstance.CommandContext(ctx, launch.executable, launch.args...)
	cmd.Dir = cwd
	cmd.Env = buildShellEnv()

	if err := cmd.Start(); err != nil {
		fmt.Printf("[remote-terminal-cloud-agent] session %s start failed: %v\n", sessionID, err)
		cancel()
		_ = ptyInstance.Close()
		callbacks.OnError(err.Error())
		return
	}

	session := &shellSession{
		pty:    ptyInstance,
		cmd:    cmd,
		cancel: cancel,
		closed: make(chan struct{}),
	}

	m.mu.Lock()
	m.sessions[sessionID] = session
	m.mu.Unlock()

	if warning != "" {
		callbacks.OnOutput("stderr", warning+"\n")
	}
	callbacks.OnReady()

	go m.pipeOutput(sessionID, session, callbacks)
	go m.waitSession(sessionID, session, callbacks)
}

func buildShellEnv() []string {
	env := os.Environ()
	env = append(env,
		"TERM="+envOrDefault("TERM", "xterm-256color"),
		"COLORTERM="+envOrDefault("COLORTERM", "truecolor"),
		"TERM_PROGRAM="+envOrDefault("TERM_PROGRAM", "remote-terminal-cloud"),
		"TERM_PROGRAM_VERSION="+envOrDefault("TERM_PROGRAM_VERSION", "agent"),
	)
	if runtime.GOOS == "windows" {
		env = append(env, "ConEmuANSI="+envOrDefault("ConEmuANSI", "ON"))
	}
	return env
}

func envOrDefault(key string, fallback string) string {
	value := strings.TrimSpace(os.Getenv(key))
	if value == "" {
		return fallback
	}
	return value
}

func resolveWorkingDirectory(workingDirectory string) (string, string) {
	normalized := strings.TrimSpace(workingDirectory)
	if normalized == "" {
		cwd, _ := os.Getwd()
		return cwd, ""
	}

	info, err := os.Stat(normalized)
	if err == nil && info.IsDir() {
		return normalized, ""
	}

	cwd, _ := os.Getwd()
	if err != nil {
		return cwd, "Unable to use working directory \"" + normalized + "\", fallback to default directory: " + err.Error()
	}

	return cwd, "Unable to use working directory \"" + normalized + "\", fallback to default directory: selected path is not a directory."
}

func (m *ShellSessionManager) pipeOutput(sessionID string, session *shellSession, callbacks ShellSessionCallbacks) {
	buffer := make([]byte, 4096)
	for {
		n, err := session.pty.Read(buffer)
		if n > 0 {
			callbacks.OnOutput("stdout", string(buffer[:n]))
		}
		if err != nil {
			if !errors.Is(err, io.EOF) && !session.isClosed() {
				callbacks.OnError(err.Error())
			}
			return
		}
	}
}

func (m *ShellSessionManager) waitSession(sessionID string, session *shellSession, callbacks ShellSessionCallbacks) {
	err := session.cmd.Wait()
	session.shutdown()

	m.mu.Lock()
	delete(m.sessions, sessionID)
	m.mu.Unlock()

	if err == nil {
		fmt.Printf("[remote-terminal-cloud-agent] session %s exited cleanly\n", sessionID)
		zero := 0
		callbacks.OnExit(&zero)
		return
	}

	if session.cmd.ProcessState != nil {
		code := session.cmd.ProcessState.ExitCode()
		fmt.Printf("[remote-terminal-cloud-agent] session %s exited with code %d\n", sessionID, code)
		callbacks.OnExit(&code)
		return
	}

	fmt.Printf("[remote-terminal-cloud-agent] session %s wait error: %v\n", sessionID, err)
	callbacks.OnError(err.Error())
	callbacks.OnExit(nil)
}

func (m *ShellSessionManager) WriteInput(sessionID string, data string) {
	m.mu.Lock()
	session := m.sessions[sessionID]
	m.mu.Unlock()
	if session == nil {
		return
	}
	_, _ = session.pty.Write([]byte(data))
}

func (m *ShellSessionManager) ResizeSession(sessionID string, cols int, rows int) {
	m.mu.Lock()
	session := m.sessions[sessionID]
	m.mu.Unlock()
	if session == nil {
		return
	}

	if cols < 1 {
		cols = 1
	}
	if rows < 1 {
		rows = 1
	}
	_ = session.pty.Resize(cols, rows)
}

func (m *ShellSessionManager) StopSession(sessionID string) {
	m.mu.Lock()
	session := m.sessions[sessionID]
	m.mu.Unlock()
	if session == nil {
		return
	}

	session.shutdown()
}

func (s *shellSession) shutdown() {
	s.close.Do(func() {
		s.cancel()
		_ = s.pty.Close()
		close(s.closed)
	})
}

func (s *shellSession) isClosed() bool {
	select {
	case <-s.closed:
		return true
	default:
		return false
	}
}

func (m *ShellSessionManager) BrowseDirectories(targetPath string) (BrowseDirectoryResult, error) {
	normalized := strings.TrimSpace(targetPath)
	if normalized == "" {
		return rootBrowseResult()
	}

	resolved, err := filepath.Abs(normalized)
	if err != nil {
		return BrowseDirectoryResult{}, err
	}

	info, err := os.Stat(resolved)
	if err != nil {
		return BrowseDirectoryResult{}, err
	}
	if !info.IsDir() {
		return BrowseDirectoryResult{}, errors.New("selected path is not a directory")
	}

	entries, err := os.ReadDir(resolved)
	if err != nil {
		return BrowseDirectoryResult{}, err
	}

	items := make([]protocol.DirectoryEntry, 0, len(entries))
	for _, entry := range entries {
		if entry.IsDir() {
			items = append(items, protocol.DirectoryEntry{
				Name: entry.Name(),
				Path: filepath.Join(resolved, entry.Name()),
			})
		}
	}
	sort.Slice(items, func(i int, j int) bool {
		return strings.ToLower(items[i].Name) < strings.ToLower(items[j].Name)
	})

	parent := filepath.Dir(resolved)
	if parent == resolved {
		parent = ""
	}

	return BrowseDirectoryResult{
		CurrentPath: resolved,
		ParentPath:  parent,
		Items:       items,
	}, nil
}

func rootBrowseResult() (BrowseDirectoryResult, error) {
	if runtime.GOOS != "windows" {
		entries, err := os.ReadDir("/")
		if err != nil {
			return BrowseDirectoryResult{}, err
		}
		items := make([]protocol.DirectoryEntry, 0, len(entries))
		for _, entry := range entries {
			if entry.IsDir() {
				items = append(items, protocol.DirectoryEntry{
					Name: entry.Name(),
					Path: filepath.Join("/", entry.Name()),
				})
			}
		}
		sort.Slice(items, func(i int, j int) bool {
			return strings.ToLower(items[i].Name) < strings.ToLower(items[j].Name)
		})
		return BrowseDirectoryResult{
			CurrentPath: "/",
			Items:       items,
		}, nil
	}

	items := make([]protocol.DirectoryEntry, 0, 26)
	for drive := 'A'; drive <= 'Z'; drive++ {
		root := string(drive) + ":\\"
		if _, err := os.Stat(root); err == nil {
			items = append(items, protocol.DirectoryEntry{
				Name: strings.TrimSuffix(root, "\\"),
				Path: root,
			})
		}
	}

	return BrowseDirectoryResult{
		CurrentPath: "",
		Items:       items,
	}, nil
}
