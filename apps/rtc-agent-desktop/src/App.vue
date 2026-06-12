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
  connected: boolean;
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

const REFRESH_INTERVAL_MS = 8000;

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
const primaryFacts = computed(() => [
  {
    key: "pid",
    label: "PID",
    value: agent.value?.pid ? String(agent.value.pid) : "--",
  },
  {
    key: "shell",
    label: "默认 Shell",
    value: status.value?.effectiveDefaultShell ?? "--",
  },
  {
    key: "token-source",
    label: "Token 来源",
    value: agent.value?.tokenSource ?? "--",
  },
]);

const hasToken = computed(() => agent.value?.hasToken ?? false);
const runtimeState = computed(() => {
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

const healthItems = computed(() => {
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
</script>

<template>
  <div class="shell">

    <!-- ── Topbar ── -->
    <header class="topbar">
      <div class="brand">
        <div class="brand-icon">RTC</div>
        <div>
          <div class="brand-name">Remote Terminal Cloud</div>
          <div class="brand-sub">Agent Desktop</div>
        </div>
      </div>

      <nav class="topbar-metrics">
        <div class="tm-item">
          <span class="tm-label">状态</span>
          <span class="tm-value" :class="{ 'c-green': runtimeState.tone==='success', 'c-red': runtimeState.tone==='danger', 'c-yellow': runtimeState.tone==='warning', 'c-blue': runtimeState.tone==='primary', 'c-dim': runtimeState.tone==='neutral' }">
            {{ runtimeState.label }}
          </span>
        </div>
        <div class="tm-item">
          <span class="tm-label">PID</span>
          <span class="tm-value" :class="agent?.pid ? 'c-green' : 'c-dim'">{{ agent?.pid ?? '--' }}</span>
        </div>
        <div class="tm-item">
          <span class="tm-label">平台</span>
          <span class="tm-value">{{ status?.platform ?? '--' }} / {{ status?.arch ?? '--' }}</span>
        </div>
        <div class="tm-item">
          <span class="tm-label">Shell</span>
          <span class="tm-value">{{ status?.effectiveDefaultShell ?? '--' }}</span>
        </div>
        <div class="tm-item">
          <span class="tm-label">Token</span>
          <span class="tm-value" :class="hasToken ? 'c-green' : 'c-red'">{{ hasToken ? '已配置' : '缺失' }}</span>
        </div>
        <div class="tm-item">
          <span class="tm-label">自启动</span>
          <span class="tm-value" :class="agent?.autostartEnabled ? 'c-blue' : 'c-dim'">{{ agent?.autostartEnabled ? 'ON' : 'OFF' }}</span>
        </div>
        <div class="tm-item">
          <span class="tm-label">版本</span>
          <span class="tm-value">{{ status?.version ?? '--' }}</span>
        </div>
      </nav>

      <div class="topbar-right">
        <span class="sync-hint">↻ {{ REFRESH_INTERVAL_MS / 1000 }}s</span>
        <span class="dot-badge" :class="agentBadgeClass">{{ agentBadge }}</span>
        <button class="btn btn-ghost" @click="refresh" :disabled="loading">{{ loading ? '...' : '刷新' }}</button>
      </div>
    </header>

    <!-- ── Notices ── -->
    <div class="notice-strip" v-if="onboardingRequired || error || feedback">
      <div v-if="onboardingRequired" class="alert alert-w">⚠ 首次启动：请填写 Token 后保存，桌面端将自动接管后台运行。</div>
      <div v-if="error"    class="alert alert-e">✕ {{ error }}</div>
      <div v-if="feedback" class="alert alert-s">✓ {{ feedback }}</div>
    </div>

    <!-- ── Main ── -->
    <div class="main-grid">

      <!-- Left column -->
      <div class="col-main">

        <!-- Agent health KPIs -->
        <div class="sec">
          <div class="sec-head">
            <span class="sec-title">Agent Health</span>
            <span class="sec-hint">{{ agent?.statusSummary ?? '正在同步...' }}</span>
          </div>
          <div class="kpi-row">
            <div v-for="item in healthItems" :key="item.key" class="kpi-cell">
              <div class="kpi-label">{{ item.label }}</div>
              <div class="kpi-val"
                :class="{ 'c-green': item.tone==='success', 'c-blue': item.tone==='primary', 'c-red': item.tone==='danger', 'c-dim': item.tone==='neutral' }">
                {{ item.value }}
              </div>
            </div>
          </div>
        </div>

        <!-- Agent control -->
        <div class="sec">
          <div class="sec-head">
            <span class="sec-title">Agent Control</span>
            <span class="sec-hint">推荐把桌面端作为唯一操作入口</span>
          </div>
          <div class="toolbar">
            <button class="btn btn-primary"       @click="runDesktopAction('start')"   :disabled="!!activeDesktopAction">{{ activeDesktopAction==='start'   ? '启动中…' : '▶ 启动' }}</button>
            <button class="btn btn-ghost"         @click="runDesktopAction('stop')"    :disabled="!!activeDesktopAction">{{ activeDesktopAction==='stop'    ? '停止中…' : '■ 停止' }}</button>
            <button class="btn btn-ghost"         @click="runDesktopAction('restart')" :disabled="!!activeDesktopAction">{{ activeDesktopAction==='restart' ? '重启中…' : '↺ 重启' }}</button>
            <button class="btn btn-danger-ghost"  @click="toggleAutostart"             :disabled="autostartBusy">
              {{ autostartBusy ? '处理中…' : agent?.autostartEnabled ? '⊘ 关闭自启' : '⊕ 开机自启' }}
            </button>
          </div>
          <div class="kpi-row" style="margin-top:10px">
            <div class="kpi-cell">
              <div class="kpi-label">期望状态</div>
              <div class="kpi-val" :class="agent?.desiredRunning ? 'c-green' : 'c-dim'">{{ agent?.desiredRunning ? 'KEEP_RUNNING' : 'ON_DEMAND' }}</div>
            </div>
            <div class="kpi-cell">
              <div class="kpi-label">进程 PID</div>
              <div class="kpi-val" :class="agent?.pid ? 'c-blue' : 'c-dim'">{{ agent?.pid ?? '--' }}</div>
            </div>
            <div class="kpi-cell">
              <div class="kpi-label">准备度</div>
              <div class="kpi-val" :class="hasToken ? 'c-green' : 'c-red'">{{ hasToken ? 'READY' : 'NEEDS_TOKEN' }}</div>
            </div>
            <div class="kpi-cell">
              <div class="kpi-label">连接态</div>
              <div class="kpi-val"
                :class="{ 'c-green': agent?.connected, 'c-blue': !agent?.connected && agent?.running, 'c-dim': !agent?.running }">
                {{ agent?.connected ? 'ONLINE' : runtimeState.label.toUpperCase() }}
              </div>
            </div>
          </div>
        </div>

        <!-- Token config -->
        <div class="sec">
          <div class="sec-head">
            <span class="sec-title">Token 接入配置</span>
            <span class="sec-hint">保存后桌面端立即接管</span>
          </div>
          <label class="field-label">Registration Token</label>
          <input v-model="token" class="field-input" type="password" autocomplete="off" placeholder="rlt_xxxxxxxxxxxxxxxxxxxxxxxx" />
          <button class="btn btn-primary btn-block" @click="saveToken" :disabled="tokenSaving">
            {{ tokenSaving ? '保存中…' : '保存并接管运行' }}
          </button>
        </div>

        <!-- Log terminal -->
        <div class="sec" style="flex:1;display:flex;flex-direction:column;">
          <div class="sec-head">
            <span class="sec-title">Runtime Console</span>
            <span class="sec-hint">后台 Agent 原始输出</span>
          </div>
          <textarea class="terminal" style="flex:1;min-height:180px"
            :value="logsText || '// 暂无日志。启动 Agent 后此处将显示注册、心跳及错误信息。'"
            readonly spellcheck="false" />
        </div>

      </div>

      <!-- Right sidebar -->
      <div class="col-side">

        <!-- Connection state -->
        <div class="sec">
          <div class="sec-head"><span class="sec-title">Connection State</span></div>
          <div class="status-block"
            :style="{ borderColor: runtimeState.tone==='success' ? 'rgba(14,203,129,.35)' : runtimeState.tone==='danger' ? 'rgba(246,70,93,.35)' : runtimeState.tone==='warning' ? 'rgba(240,185,11,.35)' : 'var(--border-hi)' }">
            <div class="status-block-title">当前连接状态</div>
            <div class="status-block-val"
              :class="{ 'c-green': runtimeState.tone==='success', 'c-red': runtimeState.tone==='danger', 'c-yellow': runtimeState.tone==='warning', 'c-blue': runtimeState.tone==='primary' }"
              style="font-size:22px">
              {{ runtimeState.label }}
            </div>
            <div class="status-block-desc">{{ runtimeState.detail }}</div>
          </div>
          <div class="health-grid" style="margin-top:8px">
            <div v-for="item in healthItems" :key="item.key" class="health-cell">
              <div class="health-label">{{ item.label }}</div>
              <div class="health-val"
                :class="{ g: item.tone==='success', b: item.tone==='primary', y: item.tone==='warning', r: item.tone==='danger', n: item.tone==='neutral' }">
                {{ item.value }}
              </div>
            </div>
          </div>
        </div>

        <!-- Environment facts -->
        <div class="sec">
          <div class="sec-head"><span class="sec-title">Environment</span></div>
          <table class="fact-table">
            <tr><td>版本</td><td>{{ status?.version ?? '--' }}</td></tr>
            <tr><td>平台</td><td>{{ status?.platform ?? '--' }}</td></tr>
            <tr><td>架构</td><td>{{ status?.arch ?? '--' }}</td></tr>
            <tr><td>Shell</td><td>{{ status?.effectiveDefaultShell ?? '--' }}</td></tr>
            <tr><td>Token 来源</td><td>{{ agent?.tokenSource ?? '--' }}</td></tr>
            <tr><td>SSH</td><td :style="{ color: status?.sshAvailable ? 'var(--green)' : 'var(--text-mute)' }">{{ status?.sshAvailable ? '可用' : '不可用' }}</td></tr>
            <tr v-if="status?.serverBaseUrl"><td>服务端</td><td style="word-break:break-all;font-size:10px">{{ status.serverBaseUrl }}</td></tr>
          </table>
        </div>

        <!-- Available shells -->
        <div class="sec" v-if="status?.availableShells?.length">
          <div class="sec-head"><span class="sec-title">Available Shells</span></div>
          <table class="fact-table">
            <tr v-for="(sh, i) in status.availableShells" :key="i">
              <td>{{ sh === status.effectiveDefaultShell ? '★' : '' }}</td>
              <td>{{ sh }}</td>
            </tr>
          </table>
        </div>

      </div>
    </div>

  </div>
</template>
