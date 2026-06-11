<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

type StatusPayload = {
  version: string;
  serverBaseUrl: string;
  registrationToken: string;
  registrationTokenSource: string;
  runHeartbeat: boolean;
  runTunnel: boolean;
  configuredDefaultShell: string;
  effectiveDefaultShell: string;
  availableShells: string[];
  sshAvailable: boolean;
  sshDetail: string;
  platform: string;
  arch: string;
};

type AgentOverview = {
  desiredRunning: boolean;
  running: boolean;
  pid?: number | null;
  autostartEnabled: boolean;
  hasToken: boolean;
  tokenSource: string;
  statusSummary: string;
};

type BootstrapPayload = {
  status: StatusPayload;
  agent: AgentOverview;
  recentLogs: AgentLogEntry[];
  desktopMode: string;
  onboardingRequired: boolean;
};

type AgentLogEntry = {
  stream: string;
  line: string;
};

type ActionPayload = {
  action?: string;
  ok: boolean;
  message?: string;
  configFile?: string;
};

type AgentActionPayload = {
  action: string;
  ok: boolean;
  message: string;
  state: AgentOverview;
};

type AutostartPayload = {
  ok: boolean;
  enabled: boolean;
  message: string;
};

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

const hasToken = computed(() => agent.value?.hasToken ?? false);
const agentBadge = computed(() => {
  if (agent.value?.running) return "Running";
  if (agent.value?.hasToken) return "Ready";
  return "Needs Token";
});
const agentBadgeClass = computed(() => {
  if (agent.value?.running) return "is-success";
  if (agent.value?.hasToken) return "is-warning";
  return "is-danger";
});
async function refresh() {
  loading.value = true;
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
      ? `Token 已保存，桌面后台代理会自动接管。`
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

onMounted(async () => {
  await refresh();
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
  unlistenState?.();
  unlistenMessage?.();
  unlistenLog?.();
});
</script>

<template>
  <main class="console-shell">
    <header class="topbar">
      <div class="topbar__brand">
        <div class="brand-mark">RTC</div>
        <div>
          <p class="brand-label">Remote Terminal Cloud Agent</p>
          <h1>桌面管理控制台</h1>
        </div>
      </div>
      <div class="topbar__actions">
        <span class="status-pill" :class="agentBadgeClass">{{ agentBadge }}</span>
        <button class="button button-secondary" @click="refresh" :disabled="loading">
          {{ loading ? "刷新中..." : "刷新状态" }}
        </button>
      </div>
    </header>

    <section class="notice-strip" v-if="onboardingRequired || error || feedback">
      <div v-if="onboardingRequired" class="alert alert-warning">
        首次启动引导：请先填写 Token，保存后桌面端会自动接管后台运行。
      </div>
      <div v-if="error" class="alert alert-danger">{{ error }}</div>
      <div v-if="feedback" class="alert alert-success">{{ feedback }}</div>
    </section>

    <section class="summary-grid">
      <article class="summary-card">
        <span class="summary-card__label">Agent 状态</span>
        <strong>{{ agent?.running ? "运行中" : "未运行" }}</strong>
        <p>{{ agent?.statusSummary ?? "正在读取后台状态" }}</p>
      </article>
      <article class="summary-card">
        <span class="summary-card__label">Token 来源</span>
        <strong>{{ agent?.tokenSource ?? "none" }}</strong>
        <p>{{ hasToken ? "已具备连接条件，可直接接管运行。" : "尚未配置 token，后台代理不会启动。" }}</p>
      </article>
      <article class="summary-card">
        <span class="summary-card__label">开机自启</span>
        <strong>{{ agent?.autostartEnabled ? "已启用" : "未启用" }}</strong>
        <p>登录当前用户后自动启动桌面管理器并驻留托盘。</p>
      </article>
    </section>

    <section class="workspace-grid">
      <article class="panel panel-span-7">
        <div class="panel__header">
          <div>
            <p class="panel__eyebrow">Agent Control</p>
            <h2>后台运行控制</h2>
          </div>
          <span class="panel__hint">推荐把桌面端作为唯一入口使用</span>
        </div>

        <div class="toolbar">
          <button
            class="button button-primary"
            @click="runDesktopAction('start')"
            :disabled="!!activeDesktopAction"
          >
            {{ activeDesktopAction === "start" ? "启动中..." : "启动后台 Agent" }}
          </button>
          <button
            class="button button-secondary"
            @click="runDesktopAction('stop')"
            :disabled="!!activeDesktopAction"
          >
            {{ activeDesktopAction === "stop" ? "停止中..." : "停止后台 Agent" }}
          </button>
          <button
            class="button button-secondary"
            @click="runDesktopAction('restart')"
            :disabled="!!activeDesktopAction"
          >
            {{ activeDesktopAction === "restart" ? "重启中..." : "重启后台 Agent" }}
          </button>
          <button class="button button-secondary" @click="toggleAutostart" :disabled="autostartBusy">
            {{ autostartBusy ? "处理中..." : agent?.autostartEnabled ? "关闭开机自启" : "启用开机自启" }}
          </button>
        </div>

        <div class="inline-kpis">
          <div class="inline-kpi">
            <span>期望状态</span>
            <strong>{{ agent?.desiredRunning ? "保持运行" : "按需启动" }}</strong>
          </div>
          <div class="inline-kpi">
            <span>当前进程 PID</span>
            <strong>{{ agent?.pid ?? "--" }}</strong>
          </div>
          <div class="inline-kpi">
            <span>连接准备度</span>
            <strong>{{ hasToken ? "已就绪" : "待配置" }}</strong>
          </div>
        </div>
      </article>

      <article class="panel panel-span-5">
        <div class="panel__header">
          <div>
            <p class="panel__eyebrow">Token</p>
            <h2>接入配置</h2>
          </div>
        </div>

        <p class="panel__desc">
          保存后会写入兼容配置文件，并立即尝试由桌面端接管后台运行。
        </p>

        <label class="field-label">Registration Token</label>
        <input
          v-model="token"
          class="field-input"
          type="password"
          autocomplete="off"
          placeholder="请输入 registration token"
        />
        <button class="button button-primary button-block" @click="saveToken" :disabled="tokenSaving">
          {{ tokenSaving ? "保存中..." : "保存并接管运行" }}
        </button>
      </article>

      <article class="panel panel-span-12">
        <div class="panel__header">
          <div>
            <p class="panel__eyebrow">Runtime Console</p>
            <h2>Agent 状态与日志终端</h2>
          </div>
          <span class="panel__hint">这里会直接显示后台 Agent 的运行输出</span>
        </div>

        <div class="terminal">
          <div v-if="!logs.length" class="terminal__empty">
            暂无日志输出。启动后台 Agent 或刷新状态后，这里会显示注册、心跳和错误信息。
          </div>
          <div v-for="(entry, index) in logs" :key="`${entry.stream}-${index}-${entry.line}`" class="terminal__line">
            <span class="terminal__stream" :class="entry.stream === 'stderr' ? 'is-stderr' : 'is-stdout'">
              {{ entry.stream }}
            </span>
            <code>{{ entry.line }}</code>
          </div>
        </div>
      </article>

    </section>
  </main>
</template>
