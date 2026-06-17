<script setup lang="ts">
import type { AgentOverview, HealthItem, RuntimeState, StatusPayload } from "../types";

defineProps<{
  status: StatusPayload | null;
  agent: AgentOverview | null;
  runtimeState: RuntimeState;
  healthItems: HealthItem[];
}>();
</script>

<template>
  <div class="col-side">
    <div class="sec">
      <div class="sec-head"><span class="sec-title">Connection State</span></div>
      <div class="status-block" :style="{ borderColor: runtimeState.tone==='success' ? 'rgba(14,203,129,.35)' : runtimeState.tone==='danger' ? 'rgba(246,70,93,.35)' : runtimeState.tone==='warning' ? 'rgba(240,185,11,.35)' : 'var(--border-hi)' }">
        <div class="status-block-title">当前连接状态</div>
        <div class="status-block-val" :class="{ 'c-green': runtimeState.tone==='success', 'c-red': runtimeState.tone==='danger', 'c-yellow': runtimeState.tone==='warning', 'c-blue': runtimeState.tone==='primary' }" style="font-size:22px">
          {{ runtimeState.label }}
        </div>
        <div class="status-block-desc">{{ runtimeState.detail }}</div>
      </div>
      <div class="health-grid" style="margin-top:8px">
        <div v-for="item in healthItems" :key="item.key" class="health-cell">
          <div class="health-label">{{ item.label }}</div>
          <div class="health-val" :class="{ g: item.tone==='success', b: item.tone==='primary', y: item.tone==='warning', r: item.tone==='danger', n: item.tone==='neutral' }">
            {{ item.value }}
          </div>
        </div>
      </div>
    </div>

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
</template>