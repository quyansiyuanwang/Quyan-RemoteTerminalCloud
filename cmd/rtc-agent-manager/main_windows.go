//go:build windows

package main

import (
	"fmt"
	"os"
	"strings"
	"unsafe"

	"github.com/remote-terminal-cloud/agent/internal/agent"
	"github.com/remote-terminal-cloud/agent/internal/protocol"
	"golang.org/x/sys/windows"
)

const (
	cwUseDefault = 0x80000000
	wsVisible    = 0x10000000
	wsChild      = 0x40000000
	wsTabStop    = 0x00010000
	wsBorder     = 0x00800000
	wsVScroll    = 0x00200000

	esReadOnly    = 0x0800
	esMultiline   = 0x0004
	esAutoVScroll = 0x0040

	bsPushButton   = 0x00000000
	bsDefPushButton = 0x00000001

	swShow = 5

	idTokenEdit      = 1002
	idSaveToken      = 1003
	idStartService   = 1004
	idStopService    = 1005
	idRestartService = 1006
	idOpenConfig     = 1007
	idOpenLogs       = 1008
	idRefresh        = 1009
	idDetailsEdit    = 1010
	idStatusLabel    = 1011
	idTokenSource    = 1012
)

var (
	user32               = windows.NewLazySystemDLL("user32.dll")
	kernel32             = windows.NewLazySystemDLL("kernel32.dll")
	advapi32             = windows.NewLazySystemDLL("advapi32.dll")
	procCreateWindowExW   = user32.NewProc("CreateWindowExW")
	procDefWindowProcW    = user32.NewProc("DefWindowProcW")
	procDispatchMessageW  = user32.NewProc("DispatchMessageW")
	procGetMessageW       = user32.NewProc("GetMessageW")
	procGetWindowTextW    = user32.NewProc("GetWindowTextW")
	procMessageBoxW       = user32.NewProc("MessageBoxW")
	procPostQuitMessage   = user32.NewProc("PostQuitMessage")
	procRegisterClassExW  = user32.NewProc("RegisterClassExW")
	procSendMessageW      = user32.NewProc("SendMessageW")
	procSetWindowTextW    = user32.NewProc("SetWindowTextW")
	procShowWindow        = user32.NewProc("ShowWindow")
	procTranslateMessage  = user32.NewProc("TranslateMessage")
	procUpdateWindow      = user32.NewProc("UpdateWindow")
	procGetModuleHandleW  = kernel32.NewProc("GetModuleHandleW")
	procOpenSCManagerW    = advapi32.NewProc("OpenSCManagerW")
	procOpenServiceW      = advapi32.NewProc("OpenServiceW")
	procStartServiceW     = advapi32.NewProc("StartServiceW")
	procControlService    = advapi32.NewProc("ControlService")
	procCloseServiceHandle = advapi32.NewProc("CloseServiceHandle")
	procQueryServiceStatus = advapi32.NewProc("QueryServiceStatus")
)

type point struct {
	X int32
	Y int32
}

type msg struct {
	HWnd    windows.Handle
	Message uint32
	WParam  uintptr
	LParam  uintptr
	Time    uint32
	Pt      point
}

type wndClassEx struct {
	Size       uint32
	Style      uint32
	WndProc    uintptr
	ClsExtra   int32
	WndExtra   int32
	Instance   windows.Handle
	Icon       windows.Handle
	Cursor     windows.Handle
	Background windows.Handle
	MenuName   *uint16
	ClassName  *uint16
	IconSm     windows.Handle
}

type serviceStatus struct {
	ServiceType      uint32
	CurrentState     uint32
	ControlsAccepted uint32
	Win32ExitCode    uint32
	ServiceExitCode  uint32
	CheckPoint       uint32
	WaitHint         uint32
}

type uiState struct {
	mainWindow   windows.Handle
	statusLabel  windows.Handle
	tokenEdit    windows.Handle
	tokenSource  windows.Handle
	detailsEdit  windows.Handle
}

var currentUI uiState

func main() {
	if err := run(); err != nil {
		showError("Remote Terminal Cloud Agent", err.Error())
		os.Exit(1)
	}
}

func run() error {
	hInstance, _, err := procGetModuleHandleW.Call(0)
	if hInstance == 0 {
		return err
	}

	className, _ := windows.UTF16PtrFromString("RTC_AGENT_MANAGER_WINDOW")
	windowTitle, _ := windows.UTF16PtrFromString("Remote Terminal Cloud Agent Manager")

	wndProc := windows.NewCallback(windowProc)
	class := wndClassEx{
		Size:      uint32(unsafe.Sizeof(wndClassEx{})),
		WndProc:   wndProc,
		Instance:  windows.Handle(hInstance),
		ClassName: className,
	}

	atom, _, regErr := procRegisterClassExW.Call(uintptr(unsafe.Pointer(&class)))
	if atom == 0 {
		return regErr
	}

	hwnd, _, createErr := procCreateWindowExW.Call(
		0,
		uintptr(unsafe.Pointer(className)),
		uintptr(unsafe.Pointer(windowTitle)),
		uintptr(0x00CF0000|wsVisible),
		cwUseDefault,
		cwUseDefault,
		900,
		680,
		0,
		0,
		hInstance,
		0,
	)
	if hwnd == 0 {
		return createErr
	}

	currentUI.mainWindow = windows.Handle(hwnd)
	procShowWindow.Call(hwnd, swShow)
	procUpdateWindow.Call(hwnd)

	var message msg
	for {
		ret, _, _ := procGetMessageW.Call(uintptr(unsafe.Pointer(&message)), 0, 0, 0)
		if int32(ret) == -1 {
			return fmt.Errorf("message loop failed")
		}
		if ret == 0 {
			break
		}
		procTranslateMessage.Call(uintptr(unsafe.Pointer(&message)))
		procDispatchMessageW.Call(uintptr(unsafe.Pointer(&message)))
	}
	return nil
}

func windowProc(hwnd uintptr, msgID uint32, wParam, lParam uintptr) uintptr {
	switch msgID {
	case 0x0001:
		createControls(windows.Handle(hwnd))
		refreshUI()
		return 0
	case 0x0111:
		handleCommand(uint16(wParam & 0xffff))
		return 0
	case 0x0002:
		procPostQuitMessage.Call(0)
		return 0
	default:
		ret, _, _ := procDefWindowProcW.Call(hwnd, uintptr(msgID), wParam, lParam)
		return ret
	}
}

func createControls(parent windows.Handle) {
	createStatic(parent, "Status", 20, 18, 120, 20, 0)
	currentUI.statusLabel = createStatic(parent, "Loading...", 120, 18, 720, 20, idStatusLabel)

	createStatic(parent, "Registration Token", 20, 54, 180, 20, 0)
	currentUI.tokenEdit = createEdit(parent, "", 20, 78, 430, 28, wsBorder|wsTabStop, idTokenEdit)
	currentUI.tokenSource = createStatic(parent, "Token source: unknown", 470, 82, 350, 20, idTokenSource)

	createButton(parent, "Save Token", 20, 124, 110, 32, bsDefPushButton, idSaveToken)
	createButton(parent, "Start Service", 142, 124, 110, 32, bsPushButton, idStartService)
	createButton(parent, "Stop Service", 264, 124, 110, 32, bsPushButton, idStopService)
	createButton(parent, "Restart Service", 386, 124, 120, 32, bsPushButton, idRestartService)
	createButton(parent, "Refresh", 518, 124, 100, 32, bsPushButton, idRefresh)
	createButton(parent, "Open Config", 630, 124, 110, 32, bsPushButton, idOpenConfig)
	createButton(parent, "Open Logs", 752, 124, 110, 32, bsPushButton, idOpenLogs)

	createStatic(parent, "Details", 20, 176, 120, 20, 0)
	currentUI.detailsEdit = createEdit(parent, "", 20, 200, 840, 410, wsBorder|wsVScroll|wsTabStop|esReadOnly|esMultiline|esAutoVScroll, idDetailsEdit)
}

func handleCommand(id uint16) {
	switch id {
	case idSaveToken:
		saveToken()
	case idStartService:
		runServiceAction("start")
	case idStopService:
		runServiceAction("stop")
	case idRestartService:
		runServiceAction("restart")
	case idOpenConfig:
		openFolderOrError(func() (string, error) {
			paths, err := agent.GetManagerPaths()
			if err != nil {
				return "", err
			}
			return paths.ConfigDir, nil
		})
	case idOpenLogs:
		openFolderOrError(func() (string, error) {
			paths, err := agent.GetManagerPaths()
			if err != nil {
				return "", err
			}
			return paths.LogsDir, nil
		})
	case idRefresh:
		refreshUI()
	}
}

func saveToken() {
	token := strings.TrimSpace(getWindowText(currentUI.tokenEdit))
	if token == "" {
		showError("Remote Terminal Cloud Agent", "Token cannot be empty.")
		return
	}
	if err := agent.SaveRegistrationToken(token); err != nil {
		showError("Remote Terminal Cloud Agent", err.Error())
		return
	}
	setWindowText(currentUI.tokenEdit, "")
	refreshUI()
	showInfo("Remote Terminal Cloud Agent", "Token saved successfully.")
}

func runServiceAction(action string) {
	ok, message := controlWindowsService(action)
	if !ok {
		showError("Remote Terminal Cloud Agent", message)
		return
	}
	refreshUI()
}

func controlWindowsService(action string) (bool, string) {
	manager, err := openServiceManager()
	if err != nil {
		return false, err.Error()
	}
	defer procCloseServiceHandle.Call(uintptr(manager))

	namePtr, _ := windows.UTF16PtrFromString("RemoteTerminalCloudAgent")
	service, _, err := procOpenServiceW.Call(uintptr(manager), uintptr(unsafe.Pointer(namePtr)), uintptr(serviceAllAccess))
	if service == 0 {
		return false, err.Error()
	}
	defer procCloseServiceHandle.Call(service)

	switch action {
	case "start":
		ret, _, err := procStartServiceW.Call(service, 0, 0)
		if ret == 0 {
			return false, err.Error()
		}
	case "stop":
		var status serviceStatus
		ret, _, err := procControlService.Call(service, uintptr(1), uintptr(unsafe.Pointer(&status)))
		if ret == 0 {
			return false, err.Error()
		}
	case "restart":
		if ok, msg := controlWindowsService("stop"); !ok {
			return false, msg
		}
		if ok, msg := controlWindowsService("start"); !ok {
			return false, msg
		}
	}

	return true, ""
}

const (
	scManagerAllAccess = 0xF003F
	serviceAllAccess   = 0xF01FF
)

func openServiceManager() (windows.Handle, error) {
	handle, _, err := procOpenSCManagerW.Call(0, 0, uintptr(scManagerAllAccess))
	if handle == 0 {
		return 0, err
	}
	return windows.Handle(handle), nil
}

func refreshUI() {
	status, err := agent.GetManagerStatus()
	if err != nil {
		setWindowText(currentUI.statusLabel, "Status: error")
		setWindowText(currentUI.detailsEdit, err.Error())
		return
	}

	setWindowText(currentUI.statusLabel, fmt.Sprintf("Status: %s | Version: %s | Platform: %s/%s", status.ServiceState, status.Version, status.Platform, status.Arch))
	setWindowText(currentUI.tokenSource, fmt.Sprintf("Token source: %s", status.TokenSource))
	setWindowText(currentUI.detailsEdit, buildDetails(status))
}

func buildDetails(status agent.ManagerStatus) string {
	tokenState := "missing"
	if status.TokenConfigured {
		tokenState = "configured"
	}

	return strings.Join([]string{
		fmt.Sprintf("Server Base URL: %s", status.ServerBaseURL),
		fmt.Sprintf("Token: %s", tokenState),
		fmt.Sprintf("Config File: %s", status.ConfigFilePath),
		fmt.Sprintf("Preferences: %s", status.PreferencesPath),
		fmt.Sprintf("Logs: %s", status.LogsDir),
		fmt.Sprintf("Heartbeat Enabled: %t", status.RunHeartbeat),
		fmt.Sprintf("Tunnel Enabled: %t", status.RunTunnel),
		fmt.Sprintf("Configured Default Shell: %s", status.ConfiguredDefaultShell),
		fmt.Sprintf("Available Shells: %s", joinShells(status.AvailableShells)),
		fmt.Sprintf("SSH Available: %t", status.SSHAvailable),
		fmt.Sprintf("SSH Detail: %s", status.SSHDetail),
	}, "\r\n")
}

func joinShells(items []protocol.ShellType) string {
	if len(items) == 0 {
		return "none"
	}
	var parts []string
	for _, item := range items {
		parts = append(parts, string(item))
	}
	return strings.Join(parts, ", ")
}

func createStatic(parent windows.Handle, text string, x, y, w, h int32, id int) windows.Handle {
	className, _ := windows.UTF16PtrFromString("STATIC")
	labelText, _ := windows.UTF16PtrFromString(text)
	handle, _, _ := procCreateWindowExW.Call(0, uintptr(unsafe.Pointer(className)), uintptr(unsafe.Pointer(labelText)), uintptr(wsVisible|wsChild), uintptr(x), uintptr(y), uintptr(w), uintptr(h), uintptr(parent), uintptr(id), 0, 0)
	return windows.Handle(handle)
}

func createButton(parent windows.Handle, text string, x, y, w, h int32, style uintptr, id int) windows.Handle {
	className, _ := windows.UTF16PtrFromString("BUTTON")
	buttonText, _ := windows.UTF16PtrFromString(text)
	handle, _, _ := procCreateWindowExW.Call(0, uintptr(unsafe.Pointer(className)), uintptr(unsafe.Pointer(buttonText)), uintptr(wsVisible|wsChild|wsTabStop|style), uintptr(x), uintptr(y), uintptr(w), uintptr(h), uintptr(parent), uintptr(id), 0, 0)
	return windows.Handle(handle)
}

func createEdit(parent windows.Handle, text string, x, y, w, h int32, style uintptr, id int) windows.Handle {
	className, _ := windows.UTF16PtrFromString("EDIT")
	editText, _ := windows.UTF16PtrFromString(text)
	handle, _, _ := procCreateWindowExW.Call(0, uintptr(unsafe.Pointer(className)), uintptr(unsafe.Pointer(editText)), uintptr(wsVisible|wsChild|style), uintptr(x), uintptr(y), uintptr(w), uintptr(h), uintptr(parent), uintptr(id), 0, 0)
	return windows.Handle(handle)
}

func setWindowText(hwnd windows.Handle, text string) {
	ptr, _ := windows.UTF16PtrFromString(text)
	procSetWindowTextW.Call(uintptr(hwnd), uintptr(unsafe.Pointer(ptr)))
}

func getWindowText(hwnd windows.Handle) string {
	buffer := make([]uint16, 2048)
	procGetWindowTextW.Call(uintptr(hwnd), uintptr(unsafe.Pointer(&buffer[0])), uintptr(len(buffer)))
	return windows.UTF16ToString(buffer)
}

func openFolderOrError(resolve func() (string, error)) {
	path, err := resolve()
	if err != nil {
		showError("Remote Terminal Cloud Agent", err.Error())
		return
	}
	_ = os.MkdirAll(path, 0o755)
	_ = windows.ShellExecute(0, windows.StringToUTF16Ptr("open"), windows.StringToUTF16Ptr("explorer.exe"), windows.StringToUTF16Ptr(path), nil, windows.SW_SHOWNORMAL)
}

func showError(title string, message string) {
	titlePtr, _ := windows.UTF16PtrFromString(title)
	messagePtr, _ := windows.UTF16PtrFromString(message)
	procMessageBoxW.Call(0, uintptr(unsafe.Pointer(messagePtr)), uintptr(unsafe.Pointer(titlePtr)), uintptr(windows.MB_OK|windows.MB_ICONERROR))
}

func showInfo(title string, message string) {
	titlePtr, _ := windows.UTF16PtrFromString(title)
	messagePtr, _ := windows.UTF16PtrFromString(message)
	procMessageBoxW.Call(0, uintptr(unsafe.Pointer(messagePtr)), uintptr(unsafe.Pointer(titlePtr)), uintptr(windows.MB_OK|windows.MB_ICONINFORMATION))
}
