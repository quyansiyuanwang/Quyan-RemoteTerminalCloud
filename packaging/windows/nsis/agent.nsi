; Remote Terminal Cloud Agent - NSIS Installer
; Build root layout (AgentBuildRoot):
;   dist\**          - compiled agent
;   runtime\node.exe - bundled Node runtime
;   service\RemoteTerminalCloudAgentService.exe
;   service\RemoteTerminalCloudAgentService.xml
;   packaging\windows\install-service.ps1
;   packaging\windows\uninstall-service.ps1
;   packaging\windows\write-config.ps1
;   packaging\windows\agent.config.json

Unicode true
SetCompressor /SOLID lzma

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "nsDialogs.nsh"

;--------------------------------
; Metadata
;--------------------------------
!ifndef AGENT_VERSION
  !define AGENT_VERSION "0.1.0"
!endif
!ifndef AGENT_BUILD_ROOT
  !error "AGENT_BUILD_ROOT must be defined (e.g. /DAGENT_BUILD_ROOT=...)"
!endif

Name "Remote Terminal Cloud Agent"
OutFile "${AGENT_BUILD_ROOT}\artifacts\windows\out\RemoteTerminalCloudAgentSetup-${AGENT_VERSION}.exe"
InstallDir "$PROGRAMFILES64\Remote Terminal Cloud Agent"
InstallDirRegKey HKLM "Software\RemoteTerminalCloudAgent" "InstallDir"
RequestExecutionLevel admin
BrandingText "Remote Terminal Cloud Agent ${AGENT_VERSION}"

;--------------------------------
; MUI pages
;--------------------------------
!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
Page custom ConfigPageCreate ConfigPageLeave
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

;--------------------------------
; Config page — collect registration token only (server URL is pre-configured)
;--------------------------------
Var RegToken
Var hRegTokenEdit

Function ConfigPageCreate
  nsDialogs::Create 1018
  Pop $0

  ${NSD_CreateLabel} 0 0 100% 12u "Registration Token:"
  ${NSD_CreatePassword} 0 14u 100% 14u ""
  Pop $hRegTokenEdit

  ${NSD_CreateLabel} 0 36u 100% 24u "Obtain the token from your server's admin panel.$\nYou can also set it later in config.json."

  nsDialogs::Show
FunctionEnd

Function ConfigPageLeave
  ${NSD_GetText} $hRegTokenEdit $RegToken
FunctionEnd

;--------------------------------
; Install
;--------------------------------
Section "Main" SecMain
  SetOutPath "$INSTDIR"

  File /r "${AGENT_BUILD_ROOT}\dist\*.*"
  File "${AGENT_BUILD_ROOT}\runtime\node.exe"
  File "${AGENT_BUILD_ROOT}\packaging\windows\install-service.ps1"
  File "${AGENT_BUILD_ROOT}\packaging\windows\uninstall-service.ps1"
  File "${AGENT_BUILD_ROOT}\packaging\windows\write-config.ps1"
  File "${AGENT_BUILD_ROOT}\packaging\windows\agent.config.json"

  SetOutPath "$INSTDIR\service"
  File "${AGENT_BUILD_ROOT}\service\RemoteTerminalCloudAgentService.exe"
  File "${AGENT_BUILD_ROOT}\service\RemoteTerminalCloudAgentService.xml"

  ; Write default config and patch token — done entirely via PowerShell to avoid
  ; NSIS path-variable expansion issues with $COMMONAPPDATA on some Windows versions.
  nsExec::ExecToLog 'powershell.exe -NonInteractive -ExecutionPolicy Bypass -Command \
    "$d=[System.Environment]::GetFolderPath(''CommonApplicationData'') + ''\RemoteTerminalCloudAgent''; \
    New-Item -ItemType Directory -Force -Path $d | Out-Null; \
    New-Item -ItemType Directory -Force -Path ($d+''\\logs'') | Out-Null; \
    Copy-Item -Force ''$INSTDIR\agent.config.json'' ($d+''\\config.json'')"'
  nsExec::ExecToLog 'powershell.exe -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\write-config.ps1" -RegToken "$RegToken"'

  ; Install and start service
  nsExec::ExecToLog 'powershell.exe -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\install-service.ps1"'

  ; Uninstaller + registry
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKLM "Software\RemoteTerminalCloudAgent" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RemoteTerminalCloudAgent" "DisplayName" "Remote Terminal Cloud Agent"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RemoteTerminalCloudAgent" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RemoteTerminalCloudAgent" "DisplayVersion" "${AGENT_VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RemoteTerminalCloudAgent" "Publisher" "Remote Terminal Cloud"
SectionEnd

;--------------------------------
; Uninstall
;--------------------------------
Section "Uninstall"
  nsExec::ExecToLog 'powershell.exe -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\uninstall-service.ps1"'
  RMDir /r "$INSTDIR"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RemoteTerminalCloudAgent"
  DeleteRegKey HKLM "Software\RemoteTerminalCloudAgent"
SectionEnd
