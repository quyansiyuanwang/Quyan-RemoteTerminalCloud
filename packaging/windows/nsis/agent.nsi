; Remote Terminal Cloud Agent - NSIS Installer
; Build root layout (AgentBuildRoot):
;   bin\rtc-agent.exe - compiled agent binary
;   bin\rtc-agent-desktop.exe - Tauri desktop manager
;   bin\rtc-agent-installer.exe - native installer helper
;   packaging\windows\agent.config.json

Unicode true
SetCompressor /SOLID lzma

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "nsDialogs.nsh"
!include "x64.nsh"

;--------------------------------
; Metadata
;--------------------------------
!ifndef AGENT_VERSION
  !define AGENT_VERSION "0.4.2"
!endif
!ifndef AGENT_BUILD_ROOT
  !error "AGENT_BUILD_ROOT must be defined (e.g. /DAGENT_BUILD_ROOT=...)"
!endif
!ifndef AGENT_OUTPUT_DIR
  !define AGENT_OUTPUT_DIR "${AGENT_BUILD_ROOT}\artifacts\windows\out"
!endif

Name "Remote Terminal Cloud Agent"
OutFile "${AGENT_OUTPUT_DIR}\RemoteTerminalCloudAgentSetup-${AGENT_VERSION}.exe"
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

Var StartMenuFolder

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

  SetOutPath "$INSTDIR\bin"
  File "${AGENT_BUILD_ROOT}\bin\rtc-agent.exe"
  File "${AGENT_BUILD_ROOT}\bin\rtc-agent-manager.exe"
  File "${AGENT_BUILD_ROOT}\bin\rtc-agent-desktop.exe"
  File "${AGENT_BUILD_ROOT}\bin\rtc-agent-installer.exe"

  SetOutPath "$INSTDIR"
  File "${AGENT_BUILD_ROOT}\packaging\windows\agent.config.json"

  ; Initialize ProgramData config directory and copy default config
  nsExec::ExecToLog '"$INSTDIR\bin\rtc-agent-installer.exe" windows init-config'

  ; Patch config with user-supplied token
  ${If} $RegToken != ""
    nsExec::ExecToLog '"$INSTDIR\bin\rtc-agent-installer.exe" windows save-token "$RegToken"'
  ${EndIf}

  StrCpy $StartMenuFolder "$SMPROGRAMS\Remote Terminal Cloud Agent"
  CreateDirectory "$StartMenuFolder"
  CreateShortCut "$StartMenuFolder\Remote Terminal Cloud Agent.lnk" "$INSTDIR\bin\rtc-agent-desktop.exe" "" "$INSTDIR\bin\rtc-agent-desktop.exe"
  CreateShortCut "$StartMenuFolder\Configure Token.lnk" "$INSTDIR\bin\rtc-agent-desktop.exe" "" "$INSTDIR\bin\rtc-agent-desktop.exe"
  CreateShortCut "$StartMenuFolder\Open Config Folder.lnk" "$INSTDIR\bin\rtc-agent-installer.exe" 'windows open-config-dir' "$INSTDIR\bin\rtc-agent-installer.exe"
  CreateShortCut "$StartMenuFolder\Open Logs.lnk" "$INSTDIR\bin\rtc-agent-installer.exe" 'windows open-logs' "$INSTDIR\bin\rtc-agent-installer.exe"
  Exec '"$INSTDIR\bin\rtc-agent-desktop.exe"'

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
  RMDir /r "$SMPROGRAMS\Remote Terminal Cloud Agent"
  RMDir /r "$INSTDIR"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RemoteTerminalCloudAgent"
  DeleteRegKey HKLM "Software\RemoteTerminalCloudAgent"
SectionEnd
