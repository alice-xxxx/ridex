<script setup lang="ts">
import { reactive, ref, watch } from "vue"
import { useTerminalStore } from "../stores/terminal"
import { useSessionStore } from "../stores/session"
import DeviceAddressPicker from "./DeviceAddressPicker.vue"
import RoundBackButton from "./RoundBackButton.vue"

const terminal = useTerminalStore()
const session = useSessionStore()
const { protocol, ota } = terminal
const createForm = () => ({ device: 2, start: 0, count: 1, valuesText: "" })
const form = reactive({
  registers: { read: createForm(), write: createForm() },
  coils: { read: createForm(), write: createForm() },
})

const activeTab = ref(ota.active || ota.result ? "ota" : "registers")

const operations = [
  {
    key: "registers",
    label: "寄存器",
    readCode: "03",
    writeCode: "06 / 16",
    valueLabel: "写入值（空格分隔，每项一个寄存器）",
    placeholder: "65535 0x0000",
  },
  {
    key: "coils",
    label: "线圈",
    readCode: "01",
    writeCode: "05 / 15",
    valueLabel: "状态值",
    placeholder: "1 0 1",
  },
] as const

const operationModes = operations.flatMap((operation) =>
  (["read", "write"] as const).map((mode) => ({ operation, mode, key: `${operation.key}-${mode}` })),
)

const resultIcons = { success: "✓", warn: "!", error: "×" }

watch(() => session.supportsOta, (supported) => {
  if (!supported && activeTab.value === "ota") activeTab.value = "registers"
})

function sendOperation(kind: "registers" | "coils", mode: "read" | "write") {
  const { device, start, count, valuesText } = form[kind][mode]
  if (mode === "read") {
    const request = { device, start, count }
    return kind === "registers" ? terminal.readRegisters(request) : terminal.readCoils(request)
  }

  const request = { device, start, valuesText }
  return kind === "registers" ? terminal.writeRegisters(request) : terminal.writeCoils(request)
}

const bytesHex = (bytes: number[] | null | undefined) =>
  bytes?.map((byte) => byte.toString(16).padStart(2, "0").toUpperCase()).join(" ") || "—"
const crcHex = (value: number) => `0x${value.toString(16).padStart(8, "0").toUpperCase()}`
const fileSize = (value: number) =>
  value < 1024 ? `${value} B` : `${(value / 1024).toFixed(1)} KiB`
const duration = (seconds: number) =>
  `${String(Math.floor(seconds / 60)).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`
</script>

<template>
  <section class="page comm-page" :class="{ 'comm-page-fill': activeTab !== 'ota' }">
    <div class="cp-workspace">
      <nav class="cp-modes" aria-label="通信操作">
        <RoundBackButton label="返回状态" @back="session.goTo('status')" />
        <button type="button" class="cp-comm-link" :class="{ active: activeTab !== 'ota' }"
          :aria-pressed="activeTab !== 'ota'" @click="activeTab = 'registers'">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"
            stroke-linejoin="round" aria-hidden="true">
            <path d="M5 4h14v12H9l-4 4V4Z" />
            <path d="M9 8h6m-6 4h4" />
          </svg>
          <span>通信</span>
        </button>
        <div class="cp-operations" aria-label="操作对象">
          <button type="button" :class="{ active: activeTab === 'registers' }"
            @click="activeTab = 'registers'">寄存器</button>
          <button type="button" :class="{ active: activeTab === 'coils' }" @click="activeTab = 'coils'">线圈</button>
        </div>
        <button v-if="session.supportsOta" type="button" class="cp-ota-link" :class="{ active: activeTab === 'ota' }"
          :aria-pressed="activeTab === 'ota'" aria-label="固件更新" title="固件更新" @click="activeTab = 'ota'">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"
            stroke-linejoin="round" aria-hidden="true">
            <path d="M12 3v12m-4-4 4 4 4-4M5 17v3h14v-3" />
          </svg>
          <span>固件更新</span>
        </button>
      </nav>

      <div class="cp-content">
        <main class="cp-canvas">
          <div class="cp-panel">
            <div v-if="activeTab !== 'ota'" class="cp-section cp-operation-grid">
              <div v-for="{ operation, mode, key } in operationModes" :key="key" class="cp-block cp-operation"
                :class="{ 'is-selected': activeTab === operation.key }">
                <div class="cp-block-head">
                  <span class="cp-block-title">{{ operation.label }}{{ mode === 'read' ? '读取' : '写入' }}</span>
                  <span class="cp-fc">{{ mode === "read" ? operation.readCode : operation.writeCode }}</span>
                </div>
                <div class="cp-fields">
                  <div class="cp-field">
                    <label>装置地址</label>
                    <DeviceAddressPicker v-model="form[operation.key][mode].device"
                      :label="`${operation.label}${mode === 'read' ? '读取' : '写入'}装置地址`" />
                  </div>
                  <div class="cp-field">
                    <label>起始地址</label>
                    <input v-model.number="form[operation.key][mode].start" type="number" min="0" max="65535"
                      class="input input-sm" />
                  </div>
                  <div v-if="mode === 'read'" class="cp-field">
                    <label>数量</label>
                    <input v-model.number="form[operation.key][mode].count" type="number" min="1"
                      class="input input-sm" />
                  </div>
                  <div v-else class="cp-field cp-field-wide">
                    <label>{{ operation.valueLabel }}</label>
                    <input v-model="form[operation.key][mode].valuesText" :placeholder="operation.placeholder"
                      class="input input-sm input-mono" />
                  </div>
                </div>
                <div class="cp-action cp-operation-action">
                  <button class="btn btn-primary" :disabled="protocol.writing"
                    @click="sendOperation(operation.key, mode)">
                    {{ mode === "read" ? "读取" : "写入" }} →
                  </button>
                </div>
                <div v-if="protocol[operation.key][mode].error" class="msg msg-error">
                  {{ protocol[operation.key][mode].error }}
                </div>
              </div>
            </div>

            <div v-show="activeTab === 'ota'" class="cp-section">
              <div class="cp-block cp-ota">
                <div class="cp-file-area" :class="{ selected: ota.fileName }">
                  <div class="cp-file-info">
                    <strong>{{ ota.fileName }}</strong>
                  </div>
                  <button class="btn btn-sm btn-outline" :disabled="ota.active" @click="terminal.selectFirmware">
                    {{ ota.fileName ? "更换文件" : "选择文件" }}
                  </button>
                </div>

                <div class="cp-config-title">
                  <span>更新参数</span><small v-if="ota.fileKind === 'packaged'">已填充，可修改</small>
                </div>
                <div class="cp-fields">
                  <div class="cp-field">
                    <label>装置地址</label>
                    <DeviceAddressPicker v-model="ota.device" label="OTA 装置地址" />
                  </div>
                  <div class="cp-field">
                    <label>厂家编码</label>
                    <input v-model="ota.manufacturer" placeholder="00003339" class="input input-sm input-mono" />
                  </div>
                  <div class="cp-field">
                    <label>更新次数</label>
                    <input v-model.number="ota.cycles" type="number" min="1" max="100" step="1" class="input input-sm"
                      :disabled="ota.active" />
                  </div>
                </div>
                <div class="cp-fields">
                  <div class="cp-field">
                    <label>硬件版本</label>
                    <input v-model="ota.hardwareVersion" placeholder="0102" class="input input-sm input-mono" />
                  </div>
                  <div class="cp-field">
                    <label>软件版本</label>
                    <input v-model="ota.softwareVersion" placeholder="0304" class="input input-sm input-mono" />
                  </div>
                </div>
                <label v-if="session.transport === 'ble'" class="cp-fast-mode">
                  <input v-model="ota.fastMode" type="checkbox" role="switch" :disabled="ota.active" />
                  快速模式
                </label>
                <dl v-if="ota.info" class="ota-meta">
                  <div>
                    <dt>产品类型</dt>
                    <dd>{{ bytesHex(ota.info.deviceType) }}</dd>
                  </div>
                  <div v-if="ota.info.description">
                    <dt>文件描述</dt>
                    <dd>{{ ota.info.description }}</dd>
                  </div>
                  <div>
                    <dt>升级内容</dt>
                    <dd>{{ fileSize(ota.info.upgradeSize) }}</dd>
                  </div>
                  <div>
                    <dt>Upgrade CRC</dt>
                    <dd>{{ crcHex(ota.info.upgradeCrc) }}</dd>
                  </div>
                  <div>
                    <dt>File CRC</dt>
                    <dd>{{ crcHex(ota.info.fileCrc) }}</dd>
                  </div>
                </dl>

                <div v-if="ota.active" class="cp-progress">
                  <div class="cp-progress-head">
                    <div class="cp-progress-status">
                      <i></i>
                      <span><small>正在更新</small><strong>{{ ota.progress }}%</strong></span>
                    </div>
                    <div class="cp-timer">
                      <small>已用时间</small>
                      <time>{{ duration(ota.elapsedSeconds) }}</time>
                    </div>
                    <strong class="cp-progress-pct">{{ ota.progress }}<small>%</small></strong>
                  </div>
                  <div class="cp-progress-track">
                    <div class="cp-progress-fill" :style="{ width: ota.progress + '%' }"></div>
                  </div>
                </div>

                <div class="cp-action cp-ota-action">
                  <span>{{
                    ota.fileName
                      ? `${ota.cycles} 次更新，每次间隔 2 秒`
                      : "选择固件后才能开始"
                  }}</span>
                  <button v-if="!ota.active" class="btn btn-primary" :disabled="!ota.fileName || protocol.writing"
                    @click="terminal.startOta">
                    开始更新 →
                  </button>
                  <button v-else class="btn btn-danger" :disabled="ota.cancelling" @click="terminal.cancelOta">
                    {{ ota.cancelling ? "正在终止…" : "终止更新" }}
                  </button>
                </div>
                <div v-if="ota.result" class="cp-result" :class="`cp-result-${ota.result.type}`">
                  <span class="cp-result-mark">{{ resultIcons[ota.result.type] }}</span>
                  <span class="cp-result-copy"><small>更新结果</small><strong>{{ ota.result.message }}</strong></span>
                  <span class="cp-result-time"><small>总用时</small><time>{{ duration(ota.elapsedSeconds) }}</time></span>
                </div>
                <div v-if="ota.error && !ota.result" class="msg msg-error">{{ ota.error }}</div>
              </div>
            </div>
          </div>
        </main>

        <aside v-if="activeTab !== 'ota'" class="cp-log">
          <div class="cp-log-head">
            <span class="cp-log-title"><i></i>通信记录 <small>{{ protocol.log.length }}</small></span>
            <button class="btn btn-xs btn-ghost" @click="terminal.clearLog">清空</button>
          </div>
          <div class="cp-log-body">
            <div v-if="!protocol.log.length" class="cp-log-empty">
              发送指令后显示响应。
            </div>
            <div v-for="entry in protocol.log" :key="entry.id" class="cp-log-entry" :class="'cp-log-' + entry.type">
              <div class="cp-log-entry-head">
                <span class="cp-log-time">{{ entry.time }}</span>
                <span class="cp-log-tag">{{ entry.label }}</span>
              </div>
              <div v-if="entry.error" class="cp-log-err">{{ entry.error }}</div>
              <template v-else>
                <div v-if="entry.request" class="cp-log-line cp-log-tx">
                  <span class="cp-log-k">TX</span><code>{{ entry.request }}</code>
                </div>
                <div v-if="entry.raw" class="cp-log-line cp-log-rx">
                  <span class="cp-log-k">RX</span><code>{{ entry.raw }}</code>
                </div>
                <div v-if="entry.data" class="cp-log-line cp-log-data">
                  <span class="cp-log-k">DATA</span><code>{{ entry.data }}</code>
                </div>
              </template>
            </div>
          </div>
        </aside>
      </div>
    </div>
  </section>
</template>

<style scoped>
.cp-block {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  min-width: 0;
  padding: clamp(1rem, 2.2vw, 1.5rem);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-md);
  background: var(--surface);
  box-shadow: none;
}

.cp-block-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.625rem;
}

.cp-block-title {
  margin-top: 0;
  font-size: 1.05rem;
  font-weight: 700;
  color: var(--text-primary);
  letter-spacing: -0.025em;
}

.cp-fc {
  min-width: 2.125rem;
  padding: 0.4375rem 0.5rem;
  border: 1px solid color-mix(in srgb, var(--primary) 22%, transparent);
  border-radius: 0.375rem;
  color: var(--primary);
  background: var(--primary-light);
  font:
    600 0.65rem/1 "SF Mono",
    "Cascadia Code",
    "Consolas",
    monospace;
  text-align: center;
}

.cp-fields {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.cp-field {
  display: flex;
  flex-direction: column;
  gap: 0.1875rem;
}

.cp-field label {
  font-size: 0.7rem;
  font-weight: 600;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.input-mono {
  font-family: "SF Mono", "Cascadia Code", "Consolas", monospace !important;
  letter-spacing: 0.02em;
}

.comm-page input[type="number"] {
  appearance: textfield;
  -moz-appearance: textfield;
}

.comm-page input[type="number"]::-webkit-inner-spin-button,
.comm-page input[type="number"]::-webkit-outer-spin-button {
  margin: 0;
  appearance: none;
}

.cp-file-area {
  display: flex;
  align-items: center;
  gap: 0.625rem;
  padding: 0.875rem;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--bg);
}

.cp-fast-mode {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  color: var(--text-secondary);
  font-size: 0.75rem;
}

.cp-fast-mode input {
  accent-color: var(--primary);
}

.ota-meta {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 1px;
  margin: 0;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--border);
}

.ota-meta div {
  min-width: 0;
  padding: 0.375rem 0.5rem;
  background: var(--bg);
}

.ota-meta dt {
  color: var(--text-muted);
  font-size: 0.62rem;
}

.ota-meta dd {
  overflow: hidden;
  margin: 0.125rem 0 0;
  color: var(--text-primary);
  font:
    600 0.7rem/1.35 "Cascadia Code",
    monospace;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cp-file-info {
  display: flex;
  align-items: flex-start;
  flex: 1 1 auto;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
  width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cp-progress {
  display: flex;
  flex-direction: column;
  gap: 0.8125rem;
  margin: 0.25rem 0;
  padding: 0.875rem 0.9375rem;
  border: 1px solid color-mix(in srgb, var(--primary) 20%, var(--border));
  border-radius: 0.5625rem;
  background: var(--primary-light);
}

.cp-progress-track {
  height: 5px;
  margin: 0;
  overflow: hidden;
  border-radius: 0.1875rem;
  background: var(--surface);
}

.cp-progress-fill {
  height: 100%;
  border-radius: 0.1875rem;
  background: var(--primary);
  transition: width 0.25s ease;
}

.cp-progress-pct {
  font-weight: 700;
  color: var(--primary);
  font-family: "SF Mono", "Cascadia Code", "Consolas", monospace;
}

.cp-log {
  position: static;
  display: flex;
  flex-direction: column;
  width: auto;
  max-height: 25rem;
  overflow: hidden;
  border: 1px solid #203846;
  border-radius: var(--radius-md);
  background: #0b1826;
  box-shadow: none;
}

.cp-log-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 2.875rem;
  padding: 0.625rem 0.875rem;
  border-bottom: 1px solid var(--border);
  background: rgba(255, 255, 255, 0.04);
  border-color: rgba(255, 255, 255, 0.08);
}

.cp-log-title {
  font-size: 0.72rem;
  font-weight: 600;
  color: #a9bcc3;
}

.cp-log-body {
  display: flex;
  flex-direction: column;
  gap: 0.375rem;
  min-height: 10.625rem;
  overflow-y: auto;
  padding: 0.625rem;
}

.cp-log-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-direction: column;
  gap: 0.4375rem;
  min-height: 8.75rem;
  color: var(--text-muted);
  font-size: 0.72rem;
  text-align: center;
}

.cp-log-entry {
  padding: 0.5rem 0.625rem;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-left-width: 3px;
  border-radius: 0.3125rem;
  background: rgba(255, 255, 255, 0.035);
  font-size: 0.72rem;
}

.cp-log-success {
  border-color: rgba(34, 197, 94, 0.2);
}

.cp-log-error {
  border-color: rgba(239, 68, 68, 0.2);
  background: rgba(239, 68, 68, 0.02);
}

.cp-log-entry-head {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 0.25rem;
}

.cp-log-time {
  font-size: 0.62rem;
  font-family: "SF Mono", "Cascadia Code", "Consolas", monospace;
  color: var(--text-muted);
}

.cp-log-tag {
  font-size: 0.65rem;
  font-weight: 600;
  color: var(--primary);
  background: var(--primary-light);
  padding: 1px 0.4375rem;
  border-radius: 0.5rem;
}

.cp-log-err {
  color: var(--danger);
  font-size: 0.72rem;
  word-break: break-all;
  line-height: 1.4;
}

.cp-log-line {
  display: flex;
  align-items: flex-start;
  gap: 0.375rem;
  margin-top: 0.1875rem;
  line-height: 1.45;
}

.cp-log-k {
  font-size: 0.6rem;
  font-weight: 700;
  color: var(--text-muted);
  flex-shrink: 0;
  padding-top: 1px;
  min-width: 2.125rem;
}

.cp-log-line code {
  font-size: 0.72rem;
  font-family: "SF Mono", "Cascadia Code", "Consolas", monospace;
  color: #6bd2cd;
  word-break: break-all;
}

.cp-log-data code {
  color: var(--success);
}

.cp-log-tx code {
  color: #f1bc72;
}

.cp-log-rx code {
  color: #6bd2cd;
}

/* Protocol bench: mode rail, command canvas, live wire trace. */
.comm-page {
  width: 100%;
  overflow: visible;
}

.cp-workspace {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  gap: 0.75rem;
}

.cp-content {
  min-width: 0;
}

.cp-canvas {
  min-width: 0;
}

.cp-panel {
  margin-top: 0;
}

.cp-modes {
  position: sticky;
  top: 0;
  z-index: 5;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.375rem 0.5rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: color-mix(in srgb, var(--surface) 86%, transparent);
}

.cp-operations {
  display: none;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  min-width: 0;
  padding: 0.15rem;
  border-radius: 0.5rem;
  background: var(--bg);
}

.cp-operations button {
  min-width: 0;
  height: 2.25rem;
  padding: 0 0.25rem;
  border: 0;
  border-radius: 0.375rem;
  color: var(--text-secondary);
  background: transparent;
  cursor: pointer;
  font-size: 0.875rem;
  font-weight: 700;
  white-space: nowrap;
}

.cp-operations button.active {
  color: var(--primary);
  background: var(--surface);
  box-shadow: var(--shadow-sm);
}

.cp-comm-link,
.cp-ota-link {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.375rem;
  height: 2.5rem;
  padding: 0 0.75rem;
  border: 1px solid var(--border-strong);
  border-radius: 0.5rem;
  color: var(--text-secondary);
  background: var(--surface-hover);
  cursor: pointer;
  font-size: 0.875rem;
  font-weight: 700;
}

.cp-comm-link.active,
.cp-ota-link.active {
  border-color: var(--primary);
  color: var(--text-inverse);
  background: var(--primary);
}

.cp-comm-link:not(.active):hover,
.cp-ota-link:not(.active):hover {
  border-color: var(--primary);
  color: var(--primary);
  background: var(--primary-light);
}

.cp-comm-link svg,
.cp-ota-link svg {
  width: 1.15rem;
  height: 1.15rem;
}

.cp-section {
  display: grid;
  gap: 0.75rem;
  margin-top: 0.75rem;
}

.cp-action {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  padding-top: 0.75rem;
  border-top: 1px solid var(--border);
}

.cp-action>span {
  color: var(--text-muted);
  font-size: 0.64rem;
}

.cp-action .btn {
  flex: 0 0 auto;
}

.cp-operation-action {
  justify-content: flex-end;
}

.cp-file-area.selected {
  border-color: color-mix(in srgb, var(--success) 40%, var(--border));
  background: var(--success-light);
}

.cp-file-info strong {
  display: block;
  width: 100%;
  max-width: 100%;
  overflow: visible;
  color: var(--text-primary);
  font-size: 0.76rem;
  line-height: 1.35;
  overflow-wrap: anywhere;
  word-break: break-all;
  white-space: normal;
}

.cp-file-info small {
  color: var(--text-muted);
  font-size: 0.6rem;
}

.cp-config-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  padding-top: 0.125rem;
}

.cp-config-title span {
  color: var(--text-secondary);
  font-size: 0.72rem;
  font-weight: 700;
}

.cp-config-title small {
  color: var(--success);
  font-size: 0.6rem;
}

.cp-progress-head {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto;
  align-items: center;
  gap: 1rem;
}

.cp-progress-status {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 0.5625rem;
}

.cp-progress-status>i {
  width: 7px;
  height: 7px;
  flex: 0 0 auto;
  border-radius: 50%;
  background: var(--primary);
  box-shadow: 0 0 0 5px color-mix(in srgb, var(--primary) 12%, transparent);
  animation: pulse 1.4s ease-in-out infinite;
}

.cp-progress-status>span {
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.cp-progress-status small,
.cp-timer small,
.cp-result-copy small,
.cp-result-time small {
  color: var(--text-muted);
  font-size: 0.54rem;
  font-weight: 700;
  letter-spacing: 0.06em;
}

.cp-progress-status strong {
  overflow: hidden;
  color: var(--text-primary);
  font-size: 0.7rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cp-timer {
  display: flex;
  align-items: flex-end;
  flex-direction: column;
  padding-right: 1rem;
  border-right: 1px solid var(--border-strong);
}

.cp-timer time {
  color: var(--text-primary);
  font:
    800 1.18rem/0.95 "Cascadia Code",
    monospace;
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.04em;
}

.cp-progress-pct {
  min-width: 3.25rem;
  color: var(--primary);
  font:
    800 1.3rem/1 "Cascadia Code",
    monospace;
  text-align: right;
}

.cp-progress-pct small {
  margin-left: 1px;
  font-size: 0.56rem;
}

.cp-result {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 0.6875rem;
  padding: 0.75rem 0.8125rem;
  border: 1px solid var(--border);
  border-radius: 0.5625rem;
  background: var(--bg);
}

.cp-result-mark {
  display: grid;
  place-items: center;
  width: 1.875rem;
  height: 1.875rem;
  border-radius: 0.4375rem;
  font-size: 0.82rem;
  font-weight: 900;
}

.cp-result-copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.cp-result-copy strong {
  overflow: visible;
  color: var(--text-primary);
  font-size: 0.7rem;
  overflow-wrap: anywhere;
  word-break: break-all;
  white-space: pre-line;
}

.cp-result-time {
  display: flex;
  align-items: flex-end;
  flex-direction: column;
  padding-left: 0.875rem;
  border-left: 1px solid var(--border);
}

.cp-result-time time {
  color: var(--text-primary);
  font:
    800 0.92rem/1 "Cascadia Code",
    monospace;
  font-variant-numeric: tabular-nums;
}

.cp-result-success {
  border-color: color-mix(in srgb, var(--success) 30%, var(--border));
  background: var(--success-light);
}

.cp-result-success .cp-result-mark {
  color: var(--success);
  background: var(--surface);
}

.cp-result-warn {
  border-color: color-mix(in srgb, var(--warning) 30%, var(--border));
  background: var(--warning-light);
}

.cp-result-warn .cp-result-mark {
  color: var(--warning);
  background: var(--surface);
}

.cp-result-error {
  border-color: color-mix(in srgb, var(--danger) 30%, var(--border));
  background: var(--danger-light);
}

.cp-result-error .cp-result-mark {
  color: var(--danger);
  background: var(--surface);
}

.cp-log-title {
  display: flex;
  align-items: center;
  gap: 0.4375rem;
}

.cp-log-title i {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #59d0aa;
  box-shadow: 0 0 8px rgba(89, 208, 170, 0.7);
}

.cp-log-title small {
  display: grid;
  place-items: center;
  min-width: 1.125rem;
  height: 1.125rem;
  padding: 0 0.25rem;
  border-radius: 0.3125rem;
  color: #6f8994;
  background: rgba(255, 255, 255, 0.05);
  font:
    600 0.55rem/1 "Cascadia Code",
    monospace;
}

.cp-log-empty {
  color: #58707b;
}

@media (min-width: 768px) {
  .cp-fields {
    flex-direction: row;
    flex-wrap: wrap;
  }

  .cp-field {
    flex: 1;
    min-width: 7.5rem;
  }

  .cp-field-wide {
    flex: 2;
  }

  .cp-operation-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .cp-operation {
    gap: 0.75rem;
    padding: 1rem;
  }

  .cp-operation .cp-block-title {
    font-size: 0.9375rem;
  }

  .cp-ota {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .cp-ota>.cp-file-area,
  .cp-ota>.cp-config-title,
  .cp-ota>.ota-meta,
  .cp-ota>.cp-progress,
  .cp-ota>.cp-ota-action,
  .cp-ota>.cp-result,
  .cp-ota>.msg {
    grid-column: 1 / -1;
  }
}

.comm-page-fill {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
}

.comm-page-fill .cp-workspace {
  flex: 1;
  min-height: 0;
}

.comm-page-fill .cp-content {
  display: grid;
  flex: 1;
  grid-template-rows: auto minmax(10rem, 1fr);
  min-height: 0;
  gap: 0.75rem;
  overflow-y: auto;
}

.comm-page-fill .cp-section {
  margin-top: 0;
}

.comm-page-fill .cp-log {
  max-height: none;
  min-height: 0;
}

.comm-page-fill .cp-log-body {
  flex: 1;
  min-height: 0;
}

.comm-page-fill .cp-log-empty {
  flex: 1;
}

.comm-page-fill .cp-log-entry {
  flex: 0 0 auto;
}

@media (max-width: 540px) {
  .cp-action {
    align-items: stretch;
    flex-direction: column;
  }

  .cp-action .btn {
    width: 100%;
  }

  .cp-file-area {
    align-items: flex-start;
    flex-wrap: wrap;
  }

  .cp-file-area .btn {
    width: 100%;
  }

  .cp-progress-head {
    gap: 0.5625rem;
  }

  .cp-progress-pct {
    min-width: 2.625rem;
    font-size: 1rem;
  }

  .cp-timer {
    padding-right: 0;
    border-right: 0;
  }

  .cp-result {
    gap: 0.5rem;
  }

  .cp-result-mark {
    width: 1.6875rem;
    height: 1.6875rem;
  }

  .cp-result-copy strong {
    white-space: normal;
  }
}

@media (max-width: 767px) {
  .comm-page {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
  }

  .cp-workspace {
    flex: 1;
    gap: 0.5rem;
  }

  .cp-modes {
    display: grid;
    flex: 0 0 auto;
    grid-template-columns: 2.25rem minmax(0, 1fr) auto;
    align-items: center;
    gap: 0.25rem;
    padding: 0.3rem;
    box-shadow: var(--shadow-sm);
  }

  .cp-modes>button:first-child {
    width: 2.25rem;
    height: 2.25rem;
    box-shadow: none;
  }

  .cp-comm-link {
    display: none;
  }

  .comm-page-fill .cp-operation:not(.is-selected) {
    display: none;
  }

  .cp-operations {
    display: grid;
  }

  .cp-operations button {
    height: 2.1rem;
    padding: 0 0.1rem;
    font-size: 0.84rem;
  }

  .cp-ota-link {
    display: grid;
    place-items: center;
    width: 2.25rem;
    height: 2.25rem;
    margin-left: 0;
    padding: 0;
  }

  .cp-ota-link span {
    display: none;
  }

  .cp-content {
    display: block;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }

  .comm-page-fill .cp-content {
    display: grid;
    grid-template-rows: auto minmax(8rem, 1fr);
    gap: 0.5rem;
  }

  .cp-operation-grid {
    grid-template-columns: minmax(0, 1fr);
    gap: 0.5rem;
  }

  .comm-page-fill .cp-operation {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 0.625rem;
    padding: 0.75rem;
  }

  .cp-operation .cp-block-head {
    grid-area: 1 / 1;
    justify-content: start;
    gap: 0.375rem;
  }

  .cp-operation .cp-block-title {
    font-size: 0.875rem;
  }

  .cp-operation .cp-fc {
    padding: 0.25rem 0.375rem;
  }

  .comm-page-fill .cp-fields {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 0.5rem;
  }

  .comm-page-fill .cp-fields:has(.cp-field-wide) {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .comm-page-fill .cp-field {
    min-width: 0;
  }

  .comm-page-fill .cp-field-wide {
    grid-column: 1 / -1;
  }

  .comm-page-fill .cp-operation-action {
    grid-area: 1 / 2;
    padding: 0;
    border: 0;
  }

  .comm-page-fill .cp-operation-action .btn {
    width: auto;
    min-height: 2.25rem;
    padding: 0.375rem 0.75rem;
    font-size: 0.8125rem;
  }

  .cp-operation>.msg {
    grid-column: 1 / -1;
  }

  .cp-field label,
  .cp-log-title {
    font-size: 0.75rem;
  }

  .cp-log-empty {
    min-height: 0;
    color: #a9bcc3;
    font-size: 0.78rem;
  }
}

@media (min-width: 1440px) {
  .cp-operation-grid {
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }

  .cp-operation .cp-fields {
    flex-direction: column;
  }
}
</style>
