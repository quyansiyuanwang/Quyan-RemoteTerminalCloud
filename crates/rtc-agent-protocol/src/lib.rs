mod host;
mod preferences;
mod session;

pub use host::{
    AgentCapabilities, AgentHeartbeatRequest, AgentHeartbeatResponse, AgentRegistrationRequest,
    AgentRegistrationResponse, DirectoryEntry, HostDiagnostics, HostSnapshot, PlatformId,
    ShellType, SshCheck,
};
pub use preferences::{
    PreferencesGetMessage, PreferencesResultMessage, PreferencesSetMessage,
    RemoteTerminalAgentPreferencesData, RemoteTerminalQuickCommandData, RemoteTerminalShortcutData,
    RemoteTerminalShortcutKind, RemoteTerminalShortcutModifier,
};
pub use session::{
    DirectoryBrowseRequestMessage, DirectoryBrowseResultMessage, SessionErrorMessage,
    SessionExitMessage, SessionInputMessage, SessionOutputMessage, SessionReadyMessage,
    SessionResizeMessage, SessionStartMessage, SessionStopMessage,
};

#[cfg(test)]
mod tests {
    use super::*;

    // ── PlatformId ──

    #[test]
    fn platform_id_serialization() {
        assert_eq!(serde_json::to_string(&PlatformId::Windows).unwrap(), "\"windows\"");
        assert_eq!(serde_json::to_string(&PlatformId::Linux).unwrap(), "\"linux\"");
        assert_eq!(serde_json::to_string(&PlatformId::Macos).unwrap(), "\"macos\"");
    }

    #[test]
    fn platform_id_deserialization() {
        assert_eq!(serde_json::from_str::<PlatformId>("\"windows\"").unwrap(), PlatformId::Windows);
        assert_eq!(serde_json::from_str::<PlatformId>("\"linux\"").unwrap(), PlatformId::Linux);
        assert_eq!(serde_json::from_str::<PlatformId>("\"macos\"").unwrap(), PlatformId::Macos);
    }

    #[test]
    fn platform_id_rejects_unknown() {
        assert!(serde_json::from_str::<PlatformId>("\"unknown\"").is_err());
    }

    // ── ShellType ──

    #[test]
    fn shell_type_as_str() {
        assert_eq!(ShellType::SystemDefault.as_str(), "system-default");
        assert_eq!(ShellType::Cmd.as_str(), "cmd");
        assert_eq!(ShellType::Powershell.as_str(), "powershell");
        assert_eq!(ShellType::Pwsh.as_str(), "pwsh");
        assert_eq!(ShellType::Bash.as_str(), "bash");
        assert_eq!(ShellType::Zsh.as_str(), "zsh");
        assert_eq!(ShellType::Sh.as_str(), "sh");
    }

    #[test]
    fn shell_type_serialization() {
        assert_eq!(serde_json::to_string(&ShellType::Bash).unwrap(), "\"bash\"");
        assert_eq!(serde_json::to_string(&ShellType::Powershell).unwrap(), "\"powershell\"");
        assert_eq!(serde_json::to_string(&ShellType::SystemDefault).unwrap(), "\"system-default\"");
    }

    #[test]
    fn shell_type_deserialization() {
        assert_eq!(serde_json::from_str::<ShellType>("\"bash\"").unwrap(), ShellType::Bash);
        assert_eq!(serde_json::from_str::<ShellType>("\"zsh\"").unwrap(), ShellType::Zsh);
    }

    #[test]
    fn shell_type_roundtrip_all_variants() {
        let variants = [
            ShellType::SystemDefault,
            ShellType::Cmd,
            ShellType::Powershell,
            ShellType::Pwsh,
            ShellType::Bash,
            ShellType::Zsh,
            ShellType::Sh,
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: ShellType = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, deserialized);
        }
    }

    // ── RemoteTerminalShortcutModifier ──

    #[test]
    fn shortcut_modifier_serialization() {
        assert_eq!(
            serde_json::to_string(&RemoteTerminalShortcutModifier::Ctrl).unwrap(),
            "\"ctrl\""
        );
        assert_eq!(serde_json::to_string(&RemoteTerminalShortcutModifier::Alt).unwrap(), "\"alt\"");
        assert_eq!(
            serde_json::to_string(&RemoteTerminalShortcutModifier::Shift).unwrap(),
            "\"shift\""
        );
        assert_eq!(
            serde_json::to_string(&RemoteTerminalShortcutModifier::Meta).unwrap(),
            "\"meta\""
        );
    }

    #[test]
    fn shortcut_modifier_deserialization() {
        assert_eq!(
            serde_json::from_str::<RemoteTerminalShortcutModifier>("\"ctrl\"").unwrap(),
            RemoteTerminalShortcutModifier::Ctrl
        );
        assert_eq!(
            serde_json::from_str::<RemoteTerminalShortcutModifier>("\"alt\"").unwrap(),
            RemoteTerminalShortcutModifier::Alt
        );
    }

    // ── RemoteTerminalShortcutKind ──

    #[test]
    fn shortcut_kind_default_is_sequence() {
        assert_eq!(RemoteTerminalShortcutKind::default(), RemoteTerminalShortcutKind::Sequence);
    }

    #[test]
    fn shortcut_kind_serialization() {
        assert_eq!(
            serde_json::to_string(&RemoteTerminalShortcutKind::Sequence).unwrap(),
            "\"sequence\""
        );
        assert_eq!(serde_json::to_string(&RemoteTerminalShortcutKind::Key).unwrap(), "\"key\"");
    }

    // ── AgentCapabilities ──

    #[test]
    fn agent_capabilities_defaults() {
        let caps = AgentCapabilities::default();
        assert!(!caps.session_recording);
    }

    #[test]
    fn agent_capabilities_roundtrip() {
        let caps = AgentCapabilities {
            ssh_forward: true,
            native_pty: true,
            self_update: false,
            proxy_aware: true,
            service_managed: false,
            session_recording: true,
        };
        let json = serde_json::to_string(&caps).unwrap();
        let deserialized: AgentCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(caps.ssh_forward, deserialized.ssh_forward);
        assert_eq!(caps.session_recording, deserialized.session_recording);
        assert_eq!(caps.self_update, deserialized.self_update);
    }

    // ── HostSnapshot serialization round-trip ──

    #[test]
    fn host_snapshot_roundtrip() {
        let snapshot = HostSnapshot {
            hostname: "test-host".into(),
            platform: Some(PlatformId::Linux),
            arch: "x86_64".into(),
            agent_version: "1.0.0".into(),
            capabilities: AgentCapabilities::default(),
            diagnostics: HostDiagnostics {
                install_formats: vec!["deb".into()],
                service_manager: "systemd".into(),
                default_log_path: "/var/log".into(),
                available_shells: vec![ShellType::Bash, ShellType::Zsh],
                ssh_check: SshCheck { available: true, detail: "ok".into() },
                notes: vec!["test note".into()],
            },
        };
        let json = serde_json::to_string_pretty(&snapshot).unwrap();
        let deserialized: HostSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.hostname, "test-host");
        assert_eq!(deserialized.platform, Some(PlatformId::Linux));
        assert_eq!(deserialized.diagnostics.available_shells.len(), 2);
    }

    #[test]
    fn host_snapshot_platform_may_be_null() {
        let json = r#"{"hostname":"h","arch":"x86_64","agentVersion":"1","capabilities":{"sshForward":true,"nativePty":true,"selfUpdate":true,"proxyAware":true,"serviceManaged":true,"sessionRecording":false},"diagnostics":{"installFormats":[],"serviceManager":"","defaultLogPath":"","availableShells":[],"sshCheck":{"available":false,"detail":""},"notes":[]}}"#;
        let snapshot: HostSnapshot = serde_json::from_str(json).unwrap();
        assert_eq!(snapshot.platform, None);
    }

    // ── Registration request/response ──

    #[test]
    fn registration_request_serialization() {
        let req = AgentRegistrationRequest {
            registration_token: "token-123".into(),
            device_fingerprint: "fp-abc".into(),
            fingerprint_version: "v1".into(),
            snapshot: HostSnapshot::default(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("token-123"));
        assert!(json.contains("fp-abc"));
    }

    #[test]
    fn registration_response_deserialization() {
        let json = r#"{"deviceId":"d-1","heartbeatIntervalSeconds":30,"heartbeatToken":"ht-1","websocketUrl":"wss://x","acceptedAt":"now"}"#;
        let resp: AgentRegistrationResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.device_id, "d-1");
        assert_eq!(resp.heartbeat_interval_seconds, 30);
        assert_eq!(resp.websocket_url, "wss://x");
    }

    #[test]
    fn registration_response_websocket_defaults_to_empty() {
        let json = r#"{"deviceId":"d-1","heartbeatIntervalSeconds":30,"heartbeatToken":"ht-1","acceptedAt":"now"}"#;
        let resp: AgentRegistrationResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.websocket_url, "");
    }

    // ── Heartbeat request/response ──

    #[test]
    fn heartbeat_response_defaults() {
        let resp = AgentHeartbeatResponse::default();
        assert!(!resp.ok);
        assert_eq!(resp.next_heartbeat_interval_seconds, 0);
    }

    // ── Session messages ──

    #[test]
    fn session_start_message_roundtrip() {
        let msg = SessionStartMessage {
            r#type: "session_start".into(),
            session_id: "sid-1".into(),
            mode: "interactive".into(),
            shell_type: ShellType::Bash,
            working_directory: "/home".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: SessionStartMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.session_id, "sid-1");
        assert_eq!(deserialized.shell_type, ShellType::Bash);
    }

    #[test]
    fn session_input_message_roundtrip() {
        let msg = SessionInputMessage {
            r#type: "session_input".into(),
            session_id: "sid-1".into(),
            data: "echo hello".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: SessionInputMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.data, "echo hello");
    }

    #[test]
    fn directory_entry_roundtrip() {
        let entry = DirectoryEntry { name: "file.txt".into(), path: "/root/file.txt".into() };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: DirectoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "file.txt");
    }

    // ── Preferences messages ──

    #[test]
    fn preferences_get_message_roundtrip() {
        let msg =
            PreferencesGetMessage { r#type: "preferences_get".into(), request_id: "r-1".into() };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: PreferencesGetMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.request_id, "r-1");
    }

    #[test]
    fn preferences_result_message_defaults() {
        let msg = PreferencesResultMessage::default();
        assert!(!msg.ok);
    }

    // ── Session output ──

    #[test]
    fn session_output_message_roundtrip() {
        let msg = SessionOutputMessage {
            r#type: "session_output".into(),
            session_id: "sid-1".into(),
            stream: "stdout".into(),
            data: "hello\n".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: SessionOutputMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.stream, "stdout");
        assert_eq!(deserialized.data, "hello\n");
    }

    #[test]
    fn session_error_message_roundtrip() {
        let json = r#"{"type":"session_error","sessionId":"sid-1","message":"not found"}"#;
        let msg: SessionErrorMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.message, "not found");
    }

    #[test]
    fn session_exit_message_roundtrip() {
        let json = r#"{"type":"session_exit","sessionId":"sid-1","exitCode":0}"#;
        let msg: SessionExitMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.exit_code, Some(0));
    }

    // ── Directory browse ──

    #[test]
    fn directory_browse_result_default_ok_is_false() {
        let msg = DirectoryBrowseResultMessage::default();
        assert!(!msg.ok);
        assert_eq!(msg.current_path, "");
    }
}
