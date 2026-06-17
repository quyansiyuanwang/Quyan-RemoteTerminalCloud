<script setup lang="ts">
import type { AgentOverview, RuntimeState, StatusPayload } from "../types";

defineProps<{
  runtimeState: RuntimeState;
  agent: AgentOverview | null;
  status: StatusPayload | null;
  hasToken: boolean;
  refreshIntervalMs: number;
  agentBadge: string;
  agentBadgeClass: string;
  loading: boolean;
}>();

const emit = defineEmits<{
  refresh: [];
}>();
</script>

<template>
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
      <span class="sync-hint">↻ {{ refreshIntervalMs / 1000 }}s</span>
      <span class="dot-badge" :class="agentBadgeClass">{{ agentBadge }}</span>
      <button class="btn btn-ghost" @click="emit('refresh')" :disabled="loading">{{ loading ? '...' : '刷新' }}</button>
    </div>
  </header>
</template>