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
Page custom ConfigPageCreate ConfigPageLeave
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

;--------------------------------
; Config page
;--------------------------------
Var ServerUrl
Var RegToken
Var hServerUrlEdit
Var hRegTokenEdit

Function ConfigPageCreate
  nsDialogs::Create 1018
  Pop $0

  ${NSD_CreateLabel} 0 0 100% 12u "Server URL:"
  ${NSD_CreateText} 0 14u 100% 14u ""
  Pop $hServerUrlEdit

  ${NSD_CreateLabel} 0 36u 100% 12u "Registration Token (optional):"
  ${NSD_CreatePassword} 0 50u 100% 14u ""
  Pop $hRegTokenEdit

  ${NSD_CreateLabel} 0 70u 100% 24u "Obtain the token from your server's admin panel.$\nYou can also set these later in config.json."

  nsDialogs::Show
FunctionEnd

Function ConfigPageLeave
  ${NSD_GetText} $hServerUrlEdit $ServerUrl
  ${NSD_GetText} $hRegTokenEdit $RegToken
  ${If} $ServerUrl == ""
    MessageBox MB_OK|MB_ICONEXCLAMATION "Server URL is required."
    Abort
  ${EndIf}
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

  SetOutPath "$INSTDIR\service"
  File "${AGENT_BUILD_ROOT}\service\RemoteTerminalCloudAgentService.exe"
  File "${AGENT_BUILD_ROOT}\service\RemoteTerminalCloudAgentService.xml"

  ; Write default config to ProgramData
  SetShellVarContext all
  CreateDirectory "$APPDATA\RemoteTerminalCloudAgent\logs"
  CopyFiles "${AGENT_BUILD_ROOT}\packaging\windows\agent.config.json" "$APPDATA\RemoteTerminalCloudAgent\config.json"

  ; Patch config with user-supplied values
  nsExec::ExecToLog 'powershell.exe -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\write-config.ps1" -ServerUrl "$ServerUrl" -RegToken "$RegToken"'

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
