; Remote Terminal Cloud Agent - NSIS Installer
; Build root layout (AgentBuildRoot):
;   bin\rtc-agent.exe - compiled agent binary
;   bin\rtc-agent-installer.exe - native installer helper
;   service\RemoteTerminalCloudAgentService.exe
;   service\RemoteTerminalCloudAgentService.xml
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
  !define AGENT_VERSION "0.2.0"
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
  InitPluginsDir
  File /oname=$PLUGINSDIR\rtc-agent-installer.exe "${AGENT_BUILD_ROOT}\bin\rtc-agent-installer.exe"
  nsExec::ExecToLog '"$PLUGINSDIR\rtc-agent-installer.exe" windows stop-service "$INSTDIR"'

  SetOutPath "$INSTDIR"

  SetOutPath "$INSTDIR\bin"
  File "${AGENT_BUILD_ROOT}\bin\rtc-agent.exe"
  File "${AGENT_BUILD_ROOT}\bin\rtc-agent-manager.exe"
  File "${AGENT_BUILD_ROOT}\bin\rtc-agent-installer.exe"

  SetOutPath "$INSTDIR"
  File "${AGENT_BUILD_ROOT}\packaging\windows\agent.config.json"

  SetOutPath "$INSTDIR\service"
  File "${AGENT_BUILD_ROOT}\service\RemoteTerminalCloudAgentService.exe"
  File "${AGENT_BUILD_ROOT}\service\RemoteTerminalCloudAgentService.xml"

  ; Initialize ProgramData config directory and copy default config
  nsExec::ExecToLog '"$INSTDIR\bin\rtc-agent-installer.exe" windows init-config'

  ; Patch config with user-supplied token
  ${If} $RegToken != ""
    nsExec::ExecToLog '"$INSTDIR\bin\rtc-agent-installer.exe" windows save-token "$RegToken"'
  ${EndIf}

  ; Install and start service
  nsExec::ExecToLog '"$INSTDIR\bin\rtc-agent-installer.exe" windows install-service "$INSTDIR" "$RegToken"'

  StrCpy $StartMenuFolder "$SMPROGRAMS\Remote Terminal Cloud Agent"
  CreateDirectory "$StartMenuFolder"
  CreateShortCut "$StartMenuFolder\Agent Manager.lnk" "$INSTDIR\bin\rtc-agent-manager.exe" "" "$INSTDIR\bin\rtc-agent-manager.exe"
  CreateShortCut "$StartMenuFolder\Configure Agent.lnk" "$INSTDIR\bin\rtc-agent-manager.exe" "" "$INSTDIR\bin\rtc-agent-manager.exe"
  CreateShortCut "$StartMenuFolder\Open Config Folder.lnk" "$INSTDIR\bin\rtc-agent-installer.exe" 'windows open-config-dir' "$INSTDIR\bin\rtc-agent-installer.exe"
  CreateShortCut "$StartMenuFolder\Open Logs.lnk" "$INSTDIR\bin\rtc-agent-installer.exe" 'windows open-logs' "$INSTDIR\bin\rtc-agent-installer.exe"

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
  nsExec::ExecToLog '"$INSTDIR\bin\rtc-agent-installer.exe" windows uninstall-service "$INSTDIR"'
  RMDir /r "$SMPROGRAMS\Remote Terminal Cloud Agent"
  RMDir /r "$INSTDIR"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\RemoteTerminalCloudAgent"
  DeleteRegKey HKLM "Software\RemoteTerminalCloudAgent"
SectionEnd
