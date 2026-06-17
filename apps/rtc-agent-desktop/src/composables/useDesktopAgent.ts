import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  ActionPayload,
  AgentActionPayload,
  AgentLogEntry,
  AgentOverview,
  AutostartPayload,
  BootstrapPayload,
  HealthItem,
  RuntimeState,
  StatusPayload,
} from "../types";

export const REFRESH_INTERVAL_MS = 8000;

export function useDesktopAgent() {
  const status = ref<StatusPayload | null>(null);
  const agent = ref<AgentOverview | null>(null);
  const token = ref("");
  const loading = ref(false);
  const tokenSaving = ref(false);
  const activeDesktopAction = ref("");
  const autostartBusy = ref(false);
  const error = ref("");
  const feedback = ref("");
  const onboardingRequired = ref(false);
  const logs = ref<AgentLogEntry[]>([]);

  const logsText = computed(() =>
    logs.value.map((entry) => `[${entry.stream}] ${entry.line}`).join("\n"),
  );

  const hasToken = computed(() => agent.value?.hasToken ?? false);

  const runtimeState = computed<RuntimeState>(() => {
    const current = agent.value;
    if (!current) {
      return {
        label: "读取中",
        tone: "neutral",
        detail: "正在同步后台状态。",
      };
    }

    const summary = current.statusSummary ?? "";
    if (current.connected) {
      return {
        label: "在线",
        tone: "success",
        detail: "注册、心跳和隧道连接均已建立。",
      };
    }
    if (summary.includes("重连") || summary.includes("中断")) {
      return {
        label: "重连中",
        tone: "warning",
        detail: "连接已中断，桌面端正在自动恢复。",
      };
    }
    if (summary.includes("等待配置") || !current.hasToken) {
      return {
        label: "待配置",
        tone: "danger",
        detail: "需要先保存 Token，后台 Agent 才能建立连接。",
      };
    }
    if (summary.includes("正在注册") || summary.includes("建立隧道")) {
      return {
        label: "注册中",
        tone: "primary",
        detail: "正在向后端注册并建立会话通道。",
      };
    }
    if (current.running) {
      return {
        label: "运行中",
        tone: "primary",
        detail: "Agent 进程已启动，等待最新连接反馈。",
      };
    }
    return {
      label: "未启动",
      tone: "neutral",
      detail: "后台 Agent 当前未运行。",
    };
  });

  const healthItems = computed<HealthItem[]>(() => {
    const current = agent.value;
    return [
      {
        key: "runtime",
        label: "运行态",
        value: current?.running ? "已启动" : "未启动",
        tone: current?.running ? "success" : "neutral",
      },
      {
        key: "connectivity",
        label: "连接态",
        value: runtimeState.value.label,
        tone: runtimeState.value.tone,
      },
      {
        key: "token",
        label: "Token",
        value: current?.hasToken ? "已配置" : "缺失",
        tone: current?.hasToken ? "success" : "danger",
      },
      {
        key: "autostart",
        label: "自启动",
        value: current?.autostartEnabled ? "已启用" : "未启用",
        tone: current?.autostartEnabled ? "primary" : "neutral",
      },
    ];
  });

  const agentBadge = computed(() => {
    if (agent.value?.connected) return "Online";
    if (agent.value?.running) return "Running";
    if (agent.value?.hasToken) return "Ready";
    return "Needs Token";
  });

  const agentBadgeClass = computed(() => {
    if (agent.value?.connected) return "is-success";
    if (agent.value?.running) return "is-primary";
    if (agent.value?.hasToken) return "is-warning";
    return "is-danger";
  });

  async function refresh() {
    const shouldShowLoading = !status.value && !agent.value;
    if (shouldShowLoading) {
      loading.value = true;
    }
    error.value = "";
    try {
      const payload = await invoke<BootstrapPayload>("desktop_bootstrap");
      status.value = payload.status;
      agent.value = payload.agent;
      logs.value = payload.recentLogs ?? [];
      onboardingRequired.value = payload.onboardingRequired;
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err);
    } finally {
      loading.value = false;
    }
  }

  async function saveToken() {
    if (!token.value.trim()) {
      error.value = "Token cannot be empty.";
      return;
    }

    tokenSaving.value = true;
    error.value = "";
    feedback.value = "";

    try {
      const result = await invoke<ActionPayload>("save_token", { token: token.value });
      feedback.value = result.ok
        ? "Token 已保存，桌面后台代理会自动接管。"
        : "Token 保存未成功。";
      token.value = "";
      await refresh();
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err);
    } finally {
      tokenSaving.value = false;
    }
  }

  async function runDesktopAction(action: "start" | "stop" | "restart" | "status") {
    activeDesktopAction.value = action;
    error.value = "";
    feedback.value = "";
    try {
      const result = await invoke<AgentActionPayload>("desktop_agent_action", { action });
      agent.value = result.state;
      feedback.value = result.message;
      await refresh();
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err);
    } finally {
      activeDesktopAction.value = "";
    }
  }

  async function toggleAutostart() {
    if (!agent.value) return;
    autostartBusy.value = true;
    error.value = "";
    feedback.value = "";
    try {
      const result = await invoke<AutostartPayload>("set_autostart", {
        enabled: !agent.value.autostartEnabled,
      });
      feedback.value = result.message;
      await refresh();
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err);
    } finally {
      autostartBusy.value = false;
    }
  }

  let unlistenState: UnlistenFn | null = null;
  let unlistenMessage: UnlistenFn | null = null;
  let unlistenLog: UnlistenFn | null = null;
  let refreshTimer: number | null = null;

  onMounted(async () => {
    await refresh();
    refreshTimer = window.setInterval(() => {
      void refresh();
    }, REFRESH_INTERVAL_MS);
    unlistenState = await listen<AgentOverview>("desktop://agent-state", (event) => {
      agent.value = event.payload;
    });
    unlistenMessage = await listen<string>("desktop://agent-message", (event) => {
      feedback.value = event.payload;
    });
    unlistenLog = await listen<AgentLogEntry>("desktop://agent-log", (event) => {
      logs.value = [...logs.value.slice(-299), event.payload];
    });
  });

  onUnmounted(() => {
    if (refreshTimer !== null) {
      window.clearInterval(refreshTimer);
      refreshTimer = null;
    }
    unlistenState?.();
    unlistenMessage?.();
    unlistenLog?.();
  });

  return {
    status,
    agent,
    token,
    loading,
    tokenSaving,
    activeDesktopAction,
    autostartBusy,
    error,
    feedback,
    onboardingRequired,
    logsText,
    hasToken,
    runtimeState,
    healthItems,
    agentBadge,
    agentBadgeClass,
    refresh,
    saveToken,
    runDesktopAction,
    toggleAutostart,
  };
}