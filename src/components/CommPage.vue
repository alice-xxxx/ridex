<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue"
import { useTerminalStore } from "../stores/terminal"
import { useSessionStore } from "../stores/session"
import DeviceAddressPicker from "./DeviceAddressPicker.vue"

const terminal = useTerminalStore()
const session = useSessionStore()
const { protocol, ota } = terminal
const form = reactive({
  registers: {
    read: { device: 1, start: 0, count: 1 },
    write: { device: 1, start: 0, valuesText: "" },
  },
  coils: {
    read: { device: 1, start: 0, count: 1 },
    write: { device: 1, start: 0, valuesText: "" },
  },
})

const activeTab = ref(ota.active || ota.result ? "ota" : "registers")

const tabs = [
  { key: "registers", label: "寄存器", hint: "03 / 06 / 16", icon: "R" },
  { key: "coils", label: "线圈", hint: "01 / 05 / 15", icon: "C" },
  { key: "ota", label: "固件更新", hint: "OTA", icon: "↑" },
]
const resultIcons = { success: "✓", warn: "!", error: "×" }
const visibleTabs = computed(() =>
  !session.supportsOta ? tabs.filter((tab) => tab.key !== "ota") : tabs,
)

watch(() => session.supportsOta, (supported) => {
  if (!supported && activeTab.value === "ota") activeTab.value = "registers"
})

const bytesHex = (bytes: number[] | null | undefined) =>
  bytes?.map((byte) => byte.toString(16).padStart(2, "0").toUpperCase()).join(" ") || "—"
const crcHex = (value: number) => `0x${value.toString(16).padStart(8, "0").toUpperCase()}`
const fileSize = (value: number) =>
  value < 1024 ? `${value} B` : `${(value / 1024).toFixed(1)} KiB`
const duration = (seconds: number) =>
  `${String(Math.floor(seconds / 60)).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`
</script>

<template>
  <section class="page comm-page">
    <header class="comm-head">
      <div>
        <span>BLE PROTOCOL WORKSPACE</span>
        <h1>协议终端</h1>
        <p>构建指令、查看响应、更新设备固件</p>
      </div>
      <span class="terminal-state"><i></i>链路已就绪</span>
    </header>
    <div class="cp-workspace">
      <nav class="cp-modes" aria-label="协议类型">
        <button
          v-for="tab in visibleTabs"
          :key="tab.key"
          class="cp-mode"
          :class="{ active: activeTab === tab.key }"
          @click="activeTab = tab.key"
        >
          <span class="cp-mode-icon">{{ tab.icon }}</span>
          <span class="cp-mode-copy"
            ><strong>{{ tab.label }}</strong
            ><small>{{ tab.hint }}</small></span
          >
        </button>
      </nav>

      <main class="cp-canvas">
        <div class="cp-panel">
          <div v-show="activeTab === 'registers'" class="cp-section">
            <div class="cp-block">
              <div class="cp-block-head">
                <div>
                  <small>READ HOLDING REGISTERS</small
                  ><span class="cp-block-title">读取寄存器</span>
                </div>
                <span class="cp-fc">03</span>
              </div>
              <div class="cp-fields">
                <div class="cp-field">
                  <label>装置地址</label>
                  <DeviceAddressPicker
                    v-model="form.registers.read.device"
                    label="读取寄存器装置地址"
                  />
                </div>
                <div class="cp-field">
                  <label>起始地址</label>
                  <input
                    v-model.number="form.registers.read.start"
                    type="number"
                    min="0"
                    max="65535"
                    class="input input-sm"
                  />
                </div>
                <div class="cp-field">
                  <label>数量</label>
                  <input
                    v-model.number="form.registers.read.count"
                    type="number"
                    min="1"
                    class="input input-sm"
                  />
                </div>
              </div>
              <div class="cp-action">
                <span>读取 {{ form.registers.read.count || 0 }} 个寄存器</span
                ><button
                  class="btn btn-primary"
                  :disabled="protocol.writing"
                  @click="terminal.readRegisters(form.registers.read)"
                >
                  发送指令 →
                </button>
              </div>
              <div v-if="protocol.registers.read.error" class="msg msg-error">
                {{ protocol.registers.read.error }}
              </div>
            </div>
            <div class="cp-block">
              <div class="cp-block-head">
                <div>
                  <small>WRITE REGISTERS</small><span class="cp-block-title">写入寄存器</span>
                </div>
                <span class="cp-fc">06 / 16</span>
              </div>
              <div class="cp-fields">
                <div class="cp-field">
                  <label>装置地址</label>
                  <DeviceAddressPicker
                    v-model="form.registers.write.device"
                    label="写入寄存器装置地址"
                  />
                </div>
                <div class="cp-field">
                  <label>起始地址</label>
                  <input
                    v-model.number="form.registers.write.start"
                    type="number"
                    min="0"
                    max="65535"
                    class="input input-sm"
                  />
                </div>
                <div class="cp-field cp-field-wide">
                  <label>写入值</label>
                  <input
                    v-model="form.registers.write.valuesText"
                    placeholder="100 0xC8 300"
                    class="input input-sm input-mono"
                  />
                </div>
              </div>
              <div class="cp-action">
                <span>十进制或 0x 十六进制，空格分隔</span
                ><button
                  class="btn btn-primary"
                  :disabled="protocol.writing"
                  @click="terminal.writeRegisters(form.registers.write)"
                >
                  发送指令 →
                </button>
              </div>
              <div v-if="protocol.registers.write.error" class="msg msg-error">
                {{ protocol.registers.write.error }}
              </div>
            </div>
          </div>

          <div v-show="activeTab === 'coils'" class="cp-section">
            <div class="cp-block">
              <div class="cp-block-head">
                <div><small>READ COILS</small><span class="cp-block-title">读取线圈</span></div>
                <span class="cp-fc">01</span>
              </div>
              <div class="cp-fields">
                <div class="cp-field">
                  <label>装置地址</label>
                  <DeviceAddressPicker
                    v-model="form.coils.read.device"
                    label="读取线圈装置地址"
                  />
                </div>
                <div class="cp-field">
                  <label>起始地址</label>
                  <input
                    v-model.number="form.coils.read.start"
                    type="number"
                    min="0"
                    max="65535"
                    class="input input-sm"
                  />
                </div>
                <div class="cp-field">
                  <label>数量</label>
                  <input
                    v-model.number="form.coils.read.count"
                    type="number"
                    min="1"
                    class="input input-sm"
                  />
                </div>
              </div>
              <div class="cp-action">
                <span>读取 {{ form.coils.read.count || 0 }} 个线圈</span
                ><button
                  class="btn btn-primary"
                  :disabled="protocol.writing"
                  @click="terminal.readCoils(form.coils.read)"
                >
                  发送指令 →
                </button>
              </div>
              <div v-if="protocol.coils.read.error" class="msg msg-error">
                {{ protocol.coils.read.error }}
              </div>
            </div>

            <div class="cp-block">
              <div class="cp-block-head">
                <div><small>WRITE COILS</small><span class="cp-block-title">写入线圈</span></div>
                <span class="cp-fc">05 / 15</span>
              </div>
              <div class="cp-fields">
                <div class="cp-field">
                  <label>装置地址</label>
                  <DeviceAddressPicker
                    v-model="form.coils.write.device"
                    label="写入线圈装置地址"
                  />
                </div>
                <div class="cp-field">
                  <label>起始地址</label>
                  <input
                    v-model.number="form.coils.write.start"
                    type="number"
                    min="0"
                    max="65535"
                    class="input input-sm"
                  />
                </div>
                <div class="cp-field cp-field-wide">
                  <label>状态值</label>
                  <input
                    v-model="form.coils.write.valuesText"
                    placeholder="1 0 1"
                    class="input input-sm input-mono"
                  />
                </div>
              </div>
              <div class="cp-action">
                <span>状态使用 1 / 0</span
                ><button
                  class="btn btn-primary"
                  :disabled="protocol.writing"
                  @click="terminal.writeCoils(form.coils.write)"
                >
                  发送指令 →
                </button>
              </div>
              <div v-if="protocol.coils.write.error" class="msg msg-error">
                {{ protocol.coils.write.error }}
              </div>
            </div>
          </div>

          <div v-show="activeTab === 'ota'" class="cp-section">
            <div class="cp-block cp-ota">
              <div class="cp-block-head">
                <div>
                  <small>FIRMWARE DELIVERY</small><span class="cp-block-title">固件更新</span>
                </div>
                <span class="cp-fc">OTA</span>
              </div>
              <div class="cp-file-area" :class="{ selected: ota.fileName }">
                <div class="cp-file-mark">{{ ota.fileName ? "FW" : "+" }}</div>
                <div class="cp-file-info">
                  <strong>{{ ota.fileName || "选择 BIN / HEX / SREC / TI-TXT / VMEM / ELF / AXF / OTA 固件" }}</strong
                  ><small>{{
                    ota.fileName
                      ? "文件已就绪，可检查参数后更新"
                      : ".ota 将自动解析并校验文件头；其他格式会转换为 BIN 后发送"
                  }}</small>
                </div>
                <button
                  class="btn btn-sm btn-outline"
                  :disabled="ota.active"
                  @click="terminal.selectFirmware"
                >
                  {{ ota.fileName ? "更换文件" : "选择文件" }}
                </button>
              </div>

              <div class="cp-config-title">
                <span>更新参数</span
                ><small v-if="ota.fileKind === 'packaged'">已从文件头填充，可修改</small>
              </div>
              <div class="cp-fields">
                <div class="cp-field">
                  <label>装置地址</label>
                  <DeviceAddressPicker v-model="ota.device" label="OTA 装置地址" />
                </div>
                <div class="cp-field">
                  <label>厂家编码</label>
                  <input
                    v-model="ota.manufacturer"
                    placeholder="00003339"
                    class="input input-sm input-mono"
                  />
                </div>
                <div class="cp-field">
                  <label>更新次数</label>
                  <input
                    v-model.number="ota.cycles"
                    type="number"
                    min="1"
                    max="100"
                    step="1"
                    class="input input-sm"
                    :disabled="ota.active"
                  />
                </div>
              </div>
              <div class="cp-fields">
                <div class="cp-field">
                  <label>硬件版本</label>
                  <input
                    v-model="ota.hardwareVersion"
                    placeholder="0102"
                    class="input input-sm input-mono"
                  />
                </div>
                <div class="cp-field">
                  <label>软件版本</label>
                  <input
                    v-model="ota.softwareVersion"
                    placeholder="0304"
                    class="input input-sm input-mono"
                  />
                </div>
              </div>
              <dl v-if="ota.info" class="ota-meta">
                <div>
                  <dt>产品类型</dt>
                  <dd>{{ bytesHex(ota.info.deviceType) }}</dd>
                </div>
                <div>
                  <dt>设备地址</dt>
                  <dd>{{ ota.info.deviceAddress ?? "不限制" }}</dd>
                </div>
                <div>
                  <dt>文件描述</dt>
                  <dd>{{ ota.info.description || "—" }}</dd>
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
                    <span
                      ><small>正在更新</small><strong>{{ ota.progress }}%</strong></span
                    >
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
                <button
                  v-if="!ota.active"
                  class="btn btn-primary"
                  :disabled="!ota.fileName || protocol.writing"
                  @click="terminal.startOta"
                >
                  开始更新 →
                </button>
                <button
                  v-else
                  class="btn btn-danger"
                  :disabled="ota.cancelling"
                  @click="terminal.cancelOta"
                >
                  {{ ota.cancelling ? "正在终止…" : "终止更新" }}
                </button>
              </div>
              <div v-if="ota.result" class="cp-result" :class="`cp-result-${ota.result.type}`">
                <span class="cp-result-mark">{{ resultIcons[ota.result.type] }}</span>
                <span class="cp-result-copy"
                  ><small>更新结果</small><strong>{{ ota.result.message }}</strong></span
                >
                <span class="cp-result-time"
                  ><small>总用时</small><time>{{ duration(ota.elapsedSeconds) }}</time></span
                >
              </div>
              <div v-if="ota.error && !ota.result" class="msg msg-error">{{ ota.error }}</div>
            </div>
          </div>
        </div>
      </main>

      <aside class="cp-log">
        <div class="cp-log-head">
          <span class="cp-log-title"
            ><i></i>实时通信流 <small>{{ protocol.log.length }}</small></span
          >
          <button class="btn btn-xs btn-ghost" @click="terminal.clearLog">清空</button>
        </div>
        <div class="cp-log-body">
          <div v-if="!protocol.log.length" class="cp-log-empty">
            <span>_</span>发送指令后，响应将在这里显示
          </div>
          <div
            v-for="entry in protocol.log"
            :key="entry.id"
            class="cp-log-entry"
            :class="'cp-log-' + entry.type"
          >
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
  </section>
</template>

<style scoped>
.comm-head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 20px;
  margin-bottom: 24px;
}
.comm-head > div > span {
  color: var(--primary);
  font:
    700 0.62rem/1 "Cascadia Code",
    monospace;
  letter-spacing: 0.14em;
}
.comm-head h1 {
  margin: 8px 0 2px;
  font-size: clamp(1.75rem, 3vw, 2.5rem);
  line-height: 1;
  letter-spacing: -0.045em;
}
.comm-head p {
  margin: 0;
  color: var(--text-muted);
  font-size: 0.72rem;
}
.terminal-state {
  display: flex;
  align-items: center;
  gap: 7px;
  color: var(--success);
  font:
    700 0.58rem/1 "Cascadia Code",
    monospace;
}
.terminal-state i {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
  box-shadow: 0 0 8px currentColor;
}

.cp-block {
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-width: 0;
  padding: clamp(16px, 2.2vw, 24px);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-md);
  background: var(--surface);
  box-shadow: none;
}

.cp-block-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.cp-block-title {
  margin-top: 6px;
  font-size: 1.05rem;
  font-weight: 700;
  color: var(--text-primary);
  letter-spacing: -0.025em;
}

.cp-fc {
  min-width: 34px;
  padding: 7px 8px;
  border: 1px solid color-mix(in srgb, var(--primary) 22%, transparent);
  border-radius: 6px;
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
  gap: 8px;
}

.cp-field {
  display: flex;
  flex-direction: column;
  gap: 3px;
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
  gap: 10px;
  padding: 14px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--bg);
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
  padding: 8px 10px;
  background: var(--bg);
}
.ota-meta dt {
  color: var(--text-muted);
  font-size: 0.62rem;
}
.ota-meta dd {
  overflow: hidden;
  margin: 2px 0 0;
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
  gap: 13px;
  margin: 4px 0;
  padding: 14px 15px;
  border: 1px solid color-mix(in srgb, var(--primary) 20%, var(--border));
  border-radius: 9px;
  background: var(--primary-light);
}

.cp-progress-track {
  height: 5px;
  margin: 0;
  overflow: hidden;
  border-radius: 3px;
  background: var(--surface);
}

.cp-progress-fill {
  height: 100%;
  border-radius: 3px;
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
  max-height: 400px;
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
  min-height: 46px;
  padding: 10px 14px;
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
  gap: 6px;
  min-height: 170px;
  overflow-y: auto;
  padding: 10px;
}

.cp-log-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-direction: column;
  gap: 7px;
  min-height: 140px;
  color: var(--text-muted);
  font-size: 0.72rem;
  text-align: center;
}

.cp-log-entry {
  padding: 8px 10px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-left-width: 3px;
  border-radius: 5px;
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
  gap: 8px;
  margin-bottom: 4px;
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
  padding: 1px 7px;
  border-radius: 8px;
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
  gap: 6px;
  margin-top: 3px;
  line-height: 1.45;
}

.cp-log-k {
  font-size: 0.6rem;
  font-weight: 700;
  color: var(--text-muted);
  flex-shrink: 0;
  padding-top: 1px;
  min-width: 34px;
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
  display: grid;
  min-width: 0;
  gap: 12px;
}
.cp-canvas {
  min-width: 0;
}
.cp-panel {
  margin-top: 0;
}
.cp-modes {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 4px;
  padding: 4px;
  overflow-x: auto;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: color-mix(in srgb, var(--surface) 86%, transparent);
}
.cp-mode {
  min-width: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 9px 10px;
  border: 0;
  border-radius: 7px;
  color: var(--text-muted);
  background: transparent;
  cursor: pointer;
  text-align: left;
  transition:
    color var(--transition-fast),
    background var(--transition-fast),
    transform var(--transition-fast);
}
.cp-mode:active {
  transform: scale(0.97);
}
.cp-mode.active {
  color: var(--text-primary);
  background: var(--surface-hover);
  box-shadow: var(--shadow-sm);
}
.cp-mode-icon {
  display: grid;
  place-items: center;
  width: 24px;
  height: 24px;
  flex: 0 0 auto;
  border: 1px solid var(--border-strong);
  border-radius: 5px;
  font:
    800 0.68rem/1 "Cascadia Code",
    monospace;
}
.cp-mode.active .cp-mode-icon {
  color: var(--text-inverse);
  border-color: var(--primary);
  background: var(--primary);
}
.cp-mode-copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
}
.cp-mode-copy strong {
  overflow: hidden;
  font-size: 0.7rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.cp-mode-copy small {
  color: var(--text-muted);
  font:
    600 0.5rem/1.4 "Cascadia Code",
    monospace;
  white-space: nowrap;
}
.cp-section {
  display: grid;
  gap: 12px;
  margin-top: 12px;
}
.cp-block-head > div {
  display: flex;
  flex-direction: column;
}
.cp-block-head > div > small {
  color: var(--primary);
  font:
    700 0.52rem/1 "Cascadia Code",
    monospace;
  letter-spacing: 0.1em;
}
.cp-action {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding-top: 12px;
  border-top: 1px solid var(--border);
}
.cp-action > span {
  color: var(--text-muted);
  font-size: 0.64rem;
}
.cp-action .btn {
  flex: 0 0 auto;
}
.cp-file-area.selected {
  border-color: color-mix(in srgb, var(--success) 40%, var(--border));
  background: var(--success-light);
}
.cp-file-mark {
  display: grid;
  place-items: center;
  width: 40px;
  height: 40px;
  flex: 0 0 auto;
  border-radius: 7px;
  color: var(--primary);
  background: var(--primary-light);
  font:
    800 0.58rem/1 "Cascadia Code",
    monospace;
}
.cp-file-area.selected .cp-file-mark {
  color: var(--success);
  background: var(--surface);
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
  gap: 12px;
  padding-top: 2px;
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
  gap: 16px;
}
.cp-progress-status {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 9px;
}
.cp-progress-status > i {
  width: 7px;
  height: 7px;
  flex: 0 0 auto;
  border-radius: 50%;
  background: var(--primary);
  box-shadow: 0 0 0 5px color-mix(in srgb, var(--primary) 12%, transparent);
  animation: pulse 1.4s ease-in-out infinite;
}
.cp-progress-status > span {
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
  padding-right: 16px;
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
  min-width: 52px;
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
  gap: 11px;
  padding: 12px 13px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--bg);
}
.cp-result-mark {
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  border-radius: 7px;
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
  padding-left: 14px;
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
  gap: 7px;
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
  min-width: 18px;
  height: 18px;
  padding: 0 4px;
  border-radius: 5px;
  color: #6f8994;
  background: rgba(255, 255, 255, 0.05);
  font:
    600 0.55rem/1 "Cascadia Code",
    monospace;
}
.cp-log-empty {
  color: #58707b;
}
.cp-log-empty span {
  color: #65cfc8;
  font:
    700 1.1rem/1 "Cascadia Code",
    monospace;
  animation: pulse 1.2s steps(2) infinite;
}
@media (min-width: 760px) {
  .cp-fields {
    flex-direction: row;
    flex-wrap: wrap;
  }

  .cp-field {
    flex: 1;
    min-width: 120px;
  }

  .cp-field-wide {
    flex: 2;
  }

  .cp-section {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .cp-section:has(.cp-block:only-child) {
    grid-template-columns: 1fr;
  }
  .cp-ota {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .cp-ota > .cp-block-head,
  .cp-ota > .cp-file-area,
  .cp-ota > .cp-config-title,
  .cp-ota > .ota-meta,
  .cp-ota > .cp-progress,
  .cp-ota > .cp-ota-action,
  .cp-ota > .cp-result,
  .cp-ota > .msg {
    grid-column: 1 / -1;
  }
}

@media (min-width: 1180px) {
  .cp-workspace {
    grid-template-columns: 112px minmax(0, 1fr) 330px;
    align-items: start;
    gap: 14px;
  }
  .cp-modes {
    position: sticky;
    top: 70px;
    grid-template-columns: 1fr;
    padding: 5px;
  }
  .cp-mode {
    justify-content: flex-start;
    padding: 11px 9px;
  }
  .cp-section {
    margin-top: 0;
  }
  .cp-log {
    position: sticky;
    top: 70px;
    max-height: calc(100vh - 110px);
  }
}

@media (max-width: 540px) {
  .comm-head p,
  .cp-mode-copy small {
    display: none;
  }
  .comm-head {
    align-items: center;
    margin-bottom: 16px;
  }
  .terminal-state {
    font-size: 0;
  }
  .terminal-state i {
    width: 8px;
    height: 8px;
  }
  .cp-mode {
    gap: 5px;
    padding: 8px 5px;
  }
  .cp-mode-icon {
    width: 21px;
    height: 21px;
  }
  .cp-mode-copy strong {
    font-size: 0.62rem;
  }
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
  .ota-meta {
    grid-template-columns: 1fr;
  }
  .cp-progress-head {
    gap: 9px;
  }
  .cp-progress-pct {
    min-width: 42px;
    font-size: 1rem;
  }
  .cp-timer {
    padding-right: 0;
    border-right: 0;
  }
  .cp-result {
    gap: 8px;
  }
  .cp-result-mark {
    width: 27px;
    height: 27px;
  }
  .cp-result-copy strong {
    white-space: normal;
  }
}

@media (max-width: 767px) {
  .comm-head {
    display: none;
  }
}
</style>
