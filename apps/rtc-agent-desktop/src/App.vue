<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

type PreferencesSummary = {
  defaultWorkingDirectory: string;
  shortcutsCount: number;
  quickCommandsCount: number;
};

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
  configFile: string;
  preferencesFile: string;
  preferencesSummary: PreferencesSummary;
};

type InstallerPaths = {
  configFile: string;
  preferencesFile: string;
  configDir: string;
  logsDir: string;
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
  installerPaths: InstallerPaths;
  agent: AgentOverview;
  desktopMode: string;
  onboardingRequired: boolean;
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

type PathPayload = {
  path: string;
};

const status = ref<StatusPayload | null>(null);
const installerPaths = ref<InstallerPaths | null>(null);
const agent = ref<AgentOverview | null>(null);
const token = ref("");
const loading = ref(false);
const tokenSaving = ref(false);
const activeDesktopAction = ref("");
const autostartBusy = ref(false);
const error = ref("");
const feedback = ref("");
const onboardingRequired = ref(false);

const shellSummary = computed(() => status.value?.availableShells.join(", ") || "none");
const hasToken = computed(() => agent.value?.hasToken ?? false);
const agentBadge = computed(() => {
  if (agent.value?.running) return "Running";
  if (agent.value?.hasToken) return "Ready";
  return "Needs Token";
});

async function refresh() {
  loading.value = true;
  error.value = "";
  try {
    const payload = await invoke<BootstrapPayload>("desktop_bootstrap");
    status.value = payload.status;
    installerPaths.value = payload.installerPaths;
    agent.value = payload.agent;
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

async function openManagedPath(kind: "configDir" | "logsDir") {
  error.value = "";
  feedback.value = "";
  try {
    const result = await invoke<PathPayload>("resolve_path", { kind });
    feedback.value = `已打开：${result.path}`;
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  }
}

let unlistenState: UnlistenFn | null = null;
let unlistenMessage: UnlistenFn | null = null;

onMounted(async () => {
  await refresh();
  unlistenState = await listen<AgentOverview>("desktop://agent-state", (event) => {
    agent.value = event.payload;
  });
  unlistenMessage = await listen<string>("desktop://agent-message", (event) => {
    feedback.value = event.payload;
  });
});

onUnmounted(() => {
  unlistenState?.();
  unlistenMessage?.();
});
</script>

<template>
  <main class="shell">
    <section class="hero">
      <div class="hero-copy">
        <p class="eyebrow">Remote Terminal Cloud Agent</p>
        <h1>桌面程序现在就是主入口</h1>
        <p class="subtle">
          安装后由桌面管理器负责托盘常驻、开机自启、后台运行 agent。
          Service 保留为可选能力，不再是普通用户的主路径。
        </p>
        <p v-if="onboardingRequired" class="hero-callout">
          首次启动引导：先保存 Token，桌面后台代理就会自动接管运行。
        </p>
      </div>
      <div class="hero-status">
        <p class="status-kicker">Desktop Mode</p>
        <strong>{{ agentBadge }}</strong>
        <span>{{ agent?.statusSummary ?? "正在读取后台状态" }}</span>
      </div>
    </section>

    <p v-if="error" class="banner banner-error">{{ error }}</p>
    <p v-if="feedback" class="banner banner-feedback">{{ feedback }}</p>

    <section class="dashboard">
      <article class="card primary-card">
        <div class="card-header">
          <div>
            <p class="section-tag">Background Agent</p>
            <h2>后台常驻</h2>
          </div>
          <button @click="refresh" :disabled="loading">{{ loading ? "刷新中" : "刷新状态" }}</button>
        </div>

        <div class="hero-grid">
          <div class="hero-panel">
            <span class="panel-label">当前状态</span>
            <strong>{{ agent?.running ? "运行中" : "未运行" }}</strong>
            <p>{{ agent?.statusSummary }}</p>
          </div>
          <div class="hero-panel">
            <span class="panel-label">开机自启</span>
            <strong>{{ agent?.autostartEnabled ? "已启用" : "未启用" }}</strong>
            <p>以当前用户身份登录后自动启动桌面管理器并最小化到托盘。</p>
          </div>
          <div class="hero-panel">
            <span class="panel-label">Token 来源</span>
            <strong>{{ agent?.tokenSource ?? "none" }}</strong>
            <p>{{ hasToken ? "后台 agent 可以直接接管运行。" : "需要先填写 token 才能启动。" }}</p>
          </div>
        </div>

        <div class="button-row">
          <button class="primary" @click="runDesktopAction('start')" :disabled="!!activeDesktopAction">
            {{ activeDesktopAction === "start" ? "启动中" : "启动后台 Agent" }}
          </button>
          <button @click="runDesktopAction('stop')" :disabled="!!activeDesktopAction">
            {{ activeDesktopAction === "stop" ? "停止中" : "停止后台 Agent" }}
          </button>
          <button @click="runDesktopAction('restart')" :disabled="!!activeDesktopAction">
            {{ activeDesktopAction === "restart" ? "重启中" : "重启后台 Agent" }}
          </button>
          <button @click="toggleAutostart" :disabled="autostartBusy">
            {{ autostartBusy ? "处理中" : agent?.autostartEnabled ? "关闭开机自启" : "启用开机自启" }}
          </button>
        </div>
      </article>

      <article class="card">
        <p class="section-tag">Token</p>
        <h2>配置接入令牌</h2>
        <p class="muted">
          保存后会写入兼容配置文件，并立即触发桌面后台代理接管，不需要用户再去命令行里手动启动。
        </p>
        <input
          v-model="token"
          type="password"
          autocomplete="off"
          placeholder="填写 registration token"
        />
        <button class="primary" @click="saveToken" :disabled="tokenSaving">
          {{ tokenSaving ? "保存中" : "保存并接管运行" }}
        </button>
      </article>

      <article class="card">
        <p class="section-tag">Overview</p>
        <h2>主机与连接状态</h2>
        <dl class="facts" v-if="status">
          <div><dt>版本</dt><dd>{{ status.version }}</dd></div>
          <div><dt>平台</dt><dd>{{ status.platform }}/{{ status.arch }}</dd></div>
          <div><dt>服务端</dt><dd>{{ status.serverBaseUrl }}</dd></div>
          <div><dt>Shells</dt><dd>{{ shellSummary }}</dd></div>
          <div><dt>SSH</dt><dd>{{ status.sshAvailable ? "可用" : "不可用" }} / {{ status.sshDetail }}</dd></div>
          <div><dt>Heartbeat</dt><dd>{{ status.runHeartbeat ? "启用" : "禁用" }}</dd></div>
          <div><dt>Tunnel</dt><dd>{{ status.runTunnel ? "启用" : "禁用" }}</dd></div>
          <div><dt>默认工作目录</dt><dd>{{ status.preferencesSummary.defaultWorkingDirectory || "未设置" }}</dd></div>
        </dl>
      </article>

      <article class="card">
        <p class="section-tag">Paths</p>
        <h2>本地入口</h2>
        <p class="path">{{ installerPaths?.configFile ?? status?.configFile }}</p>
        <p class="path">{{ installerPaths?.preferencesFile ?? status?.preferencesFile }}</p>
        <p class="path">{{ installerPaths?.logsDir }}</p>
        <div class="button-row">
          <button @click="openManagedPath('configDir')">打开配置目录</button>
          <button @click="openManagedPath('logsDir')">打开日志目录</button>
        </div>
      </article>

    </section>
  </main>
</template>
