<script setup lang="ts">
import type { AgentOverview, HealthItem, RuntimeState, StatusPayload } from "../types";

defineProps<{
  agent: AgentOverview | null;
  status: StatusPayload | null;
  runtimeState: RuntimeState;
  healthItems: HealthItem[];
  hasToken: boolean;
  token: string;
  tokenSaving: boolean;
  activeDesktopAction: string;
  autostartBusy: boolean;
  logsText: string;
}>();

const emit = defineEmits<{
  updateToken: [value: string];
  saveToken: [];
  runAction: [action: "start" | "stop" | "restart"];
  toggleAutostart: [];
}>();
</script>

<template>
  <div class="col-main">
    <div class="sec">
      <div class="sec-head">
        <span class="sec-title">Agent Health</span>
        <span class="sec-hint">{{ agent?.statusSummary ?? '正在同步...' }}</span>
      </div>
      <div class="kpi-row">
        <div v-for="item in healthItems" :key="item.key" class="kpi-cell">
          <div class="kpi-label">{{ item.label }}</div>
          <div class="kpi-val" :class="{ 'c-green': item.tone==='success', 'c-blue': item.tone==='primary', 'c-red': item.tone==='danger', 'c-dim': item.tone==='neutral' }">
            {{ item.value }}
          </div>
        </div>
      </div>
    </div>

    <div class="sec">
      <div class="sec-head">
        <span class="sec-title">Agent Control</span>
        <span class="sec-hint">推荐把桌面端作为唯一操作入口</span>
      </div>
      <div class="toolbar">
        <button class="btn btn-primary" @click="emit('runAction', 'start')" :disabled="!!activeDesktopAction">{{ activeDesktopAction==='start' ? '启动中…' : '▶ 启动' }}</button>
        <button class="btn btn-ghost" @click="emit('runAction', 'stop')" :disabled="!!activeDesktopAction">{{ activeDesktopAction==='stop' ? '停止中…' : '■ 停止' }}</button>
        <button class="btn btn-ghost" @click="emit('runAction', 'restart')" :disabled="!!activeDesktopAction">{{ activeDesktopAction==='restart' ? '重启中…' : '↺ 重启' }}</button>
        <button class="btn btn-danger-ghost" @click="emit('toggleAutostart')" :disabled="autostartBusy">
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
          <div class="kpi-val" :class="{ 'c-green': agent?.connected, 'c-blue': !agent?.connected && agent?.running, 'c-dim': !agent?.running }">
            {{ agent?.connected ? 'ONLINE' : runtimeState.label.toUpperCase() }}
          </div>
        </div>
      </div>
    </div>

    <div class="sec">
      <div class="sec-head">
        <span class="sec-title">Token 接入配置</span>
        <span class="sec-hint">保存后桌面端立即接管</span>
      </div>
      <label class="field-label">Registration Token</label>
      <input :value="token" class="field-input" type="password" autocomplete="off" placeholder="rtm_xxxxxxxxxxxxxxxxxxxxxxxx" @input="emit('updateToken', ($event.target as HTMLInputElement).value)" />
      <button class="btn btn-primary btn-block" @click="emit('saveToken')" :disabled="tokenSaving">
        {{ tokenSaving ? '保存中…' : '保存并接管运行' }}
      </button>
    </div>

    <div class="sec" style="flex:1;display:flex;flex-direction:column;">
      <div class="sec-head">
        <span class="sec-title">Runtime Console</span>
        <span class="sec-hint">后台 Agent 原始输出</span>
      </div>
      <textarea class="terminal" style="flex:1;min-height:180px" :value="logsText || '// 暂无日志。启动 Agent 后此处将显示注册、心跳及错误信息。'" readonly spellcheck="false" />
    </div>
  </div>
</template>