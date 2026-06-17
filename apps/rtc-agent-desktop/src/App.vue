<script setup lang="ts">
import DesktopMainPanel from "./components/DesktopMainPanel.vue";
import DesktopSidebar from "./components/DesktopSidebar.vue";
import DesktopTopbar from "./components/DesktopTopbar.vue";
import { REFRESH_INTERVAL_MS, useDesktopAgent } from "./composables/useDesktopAgent";

const {
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
} = useDesktopAgent();
</script>

<template>
  <div class="shell">

    <!-- ── Topbar ── -->
    <DesktopTopbar
      :runtime-state="runtimeState"
      :agent="agent"
      :status="status"
      :has-token="hasToken"
      :refresh-interval-ms="REFRESH_INTERVAL_MS"
      :agent-badge="agentBadge"
      :agent-badge-class="agentBadgeClass"
      :loading="loading"
      @refresh="refresh"
    />

    <!-- ── Notices ── -->
    <div class="notice-strip" v-if="onboardingRequired || error || feedback">
      <div v-if="onboardingRequired" class="alert alert-w">⚠ 首次启动：请填写 Token 后保存，桌面端将自动接管后台运行。</div>
      <div v-if="error"    class="alert alert-e">✕ {{ error }}</div>
      <div v-if="feedback" class="alert alert-s">✓ {{ feedback }}</div>
    </div>

    <!-- ── Main ── -->
    <div class="main-grid">

      <!-- Left column -->
      <DesktopMainPanel
        :agent="agent"
        :status="status"
        :runtime-state="runtimeState"
        :health-items="healthItems"
        :has-token="hasToken"
        :token="token"
        :token-saving="tokenSaving"
        :active-desktop-action="activeDesktopAction"
        :autostart-busy="autostartBusy"
        :logs-text="logsText"
        @update-token="(value) => (token = value)"
        @save-token="saveToken"
        @run-action="runDesktopAction"
        @toggle-autostart="toggleAutostart"
      />

      <!-- Right sidebar -->
      <DesktopSidebar
        :status="status"
        :agent="agent"
        :runtime-state="runtimeState"
        :health-items="healthItems"
      />
    </div>

  </div>
</template>
