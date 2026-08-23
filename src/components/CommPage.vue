<script setup>
import { computed, ref, watch } from "vue"
import { storeToRefs } from "pinia"
import { deviceAddresses } from "../registers/components.js"
import { useTerminalStore } from "../stores/terminal.js"

const terminal = useTerminalStore()
const {
  writing, readDev, readStart, readCount, readRegResp,
  writeDev, writeStart, writeValues, writeRegResp,
  readCoilDev, readCoilStart, readCoilCount, readCoilResp,
  writeCoilDev, writeCoilStart, writeCoilStates, writeCoilResp,
  otaFileName, otaDev, otaManufacturer, otaHwVersion, otaSwVersion, fileKind, otaInfo, cycles,
  otaActive, otaCancelling, otaProgress,
  otaElapsedSeconds, otaError, otaResult, supportsOta, commLog,
} = storeToRefs(terminal)

const activeTab = ref(otaActive.value || otaResult.value ? "ota" : "registers")

const tabs = [
  { key: "registers", label: "寄存器", hint: "03 / 06 / 16", icon: "R" },
  { key: "coils", label: "线圈", hint: "01 / 05 / 15", icon: "C" },
  { key: "ota", label: "固件更新", hint: "OTA", icon: "↑" },
]
const deviceFields = {
  read: readDev,
  write: writeDev,
  readCoil: readCoilDev,
  writeCoil: writeCoilDev,
  ota: otaDev,
}
const openDeviceMenu = ref("")

function toggleDeviceMenu(field) {
  openDeviceMenu.value = openDeviceMenu.value === field ? "" : field
}

function selectDevice(field, value) {
  deviceFields[field].value = value
  openDeviceMenu.value = ""
}

const visibleTabs = computed(() => !supportsOta.value
  ? tabs.filter((tab) => tab.key !== "ota")
  : tabs)

watch(supportsOta, (supported) => {
  if (!supported && activeTab.value === "ota") activeTab.value = "registers"
})

const bytesHex = (bytes) => bytes?.map((byte) => byte.toString(16).padStart(2, "0").toUpperCase()).join(" ") || "—"
const crcHex = (value) => `0x${value.toString(16).padStart(8, "0").toUpperCase()}`
const fileSize = (value) => value < 1024 ? `${value} B` : `${(value / 1024).toFixed(1)} KiB`
const duration = (seconds) => `${String(Math.floor(seconds / 60)).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`
</script>

<template>
  <section class="page comm-page" @click="openDeviceMenu = ''" @keydown.esc="openDeviceMenu = ''">
    <header class="comm-head">
      <div><span>BLE PROTOCOL WORKSPACE</span><h1>协议终端</h1><p>构建指令、查看响应、更新设备固件</p></div>
      <span class="terminal-state"><i></i>链路已就绪</span>
    </header>
    <div class="cp-workspace">
      <nav class="cp-modes" aria-label="协议类型">
        <button v-for="tab in visibleTabs" :key="tab.key" class="cp-mode" :class="{ active: activeTab === tab.key }" @click="activeTab = tab.key">
          <span class="cp-mode-icon">{{ tab.icon }}</span>
          <span class="cp-mode-copy"><strong>{{ tab.label }}</strong><small>{{ tab.hint }}</small></span>
        </button>
      </nav>

      <main class="cp-canvas">
        <div class="cp-panel">
          <div v-show="activeTab === 'registers'" class="cp-section">
            <div class="cp-block">
              <div class="cp-block-head">
                <div><small>READ HOLDING REGISTERS</small><span class="cp-block-title">读取寄存器</span></div>
                <span class="cp-fc">03</span>
              </div>
              <div class="cp-fields">
                <div class="cp-field">
                  <label>装置地址</label>
                  <div class="cp-device-picker" @click.stop>
                    <input v-model.number="readDev" type="number" min="0" max="255" class="input input-sm cp-device-input" aria-label="读取寄存器装置地址" @focus="openDeviceMenu = ''" />
                    <button class="cp-device-toggle" type="button" :aria-expanded="openDeviceMenu === 'read'" aria-label="选择读取寄存器装置" @click="toggleDeviceMenu('read')">▾</button>
                    <div v-if="openDeviceMenu === 'read'" class="cp-device-menu">
                      <button v-for="device in deviceAddresses" :key="device.value" type="button" class="cp-device-option" :class="{ selected: readDev === device.value }" @click="selectDevice('read', device.value)">
                        <strong>{{ device.value }}</strong><span>{{ device.label }}</span>
                      </button>
                      <small class="cp-device-hint">可直接输入自定义地址</small>
                    </div>
                  </div>
                </div>
                <div class="cp-field">
                  <label>起始地址</label>
                  <input v-model.number="readStart" type="number" min="0" max="65535" class="input input-sm" />
                </div>
                <div class="cp-field">
                  <label>数量</label>
                  <input v-model.number="readCount" type="number" min="1" class="input input-sm" />
                </div>
              </div>
              <div class="cp-action"><span>读取 {{ readCount || 0 }} 个寄存器</span><button class="btn btn-primary" :disabled="writing" @click="terminal.sendReadRegisters">发送指令 →</button></div>
              <div v-if="readRegResp.error" class="msg msg-error">{{ readRegResp.error }}</div>
            </div>
            <div class="cp-block">
              <div class="cp-block-head">
                <div><small>WRITE REGISTERS</small><span class="cp-block-title">写入寄存器</span></div>
                <span class="cp-fc">06 / 16</span>
              </div>
              <div class="cp-fields">
                <div class="cp-field">
                  <label>装置地址</label>
                  <div class="cp-device-picker" @click.stop>
                    <input v-model.number="writeDev" type="number" min="0" max="255" class="input input-sm cp-device-input" aria-label="写入寄存器装置地址" @focus="openDeviceMenu = ''" />
                    <button class="cp-device-toggle" type="button" :aria-expanded="openDeviceMenu === 'write'" aria-label="选择写入寄存器装置" @click="toggleDeviceMenu('write')">▾</button>
                    <div v-if="openDeviceMenu === 'write'" class="cp-device-menu">
                      <button v-for="device in deviceAddresses" :key="device.value" type="button" class="cp-device-option" :class="{ selected: writeDev === device.value }" @click="selectDevice('write', device.value)">
                        <strong>{{ device.value }}</strong><span>{{ device.label }}</span>
                      </button>
                      <small class="cp-device-hint">可直接输入自定义地址</small>
                    </div>
                  </div>
                </div>
                <div class="cp-field">
                  <label>起始地址</label>
                  <input v-model.number="writeStart" type="number" min="0" max="65535" class="input input-sm" />
                </div>
                <div class="cp-field cp-field-wide">
                  <label>写入值</label>
                  <input v-model="writeValues" placeholder="0064 00C8 012C" class="input input-sm input-mono" />
                </div>
              </div>
              <div class="cp-action"><span>HEX 字以空格分隔</span><button class="btn btn-primary" :disabled="writing" @click="terminal.sendWriteRegisters">发送指令 →</button></div>
              <div v-if="writeRegResp.error" class="msg msg-error">{{ writeRegResp.error }}</div>
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
                  <div class="cp-device-picker" @click.stop>
                    <input v-model.number="readCoilDev" type="number" min="0" max="255" class="input input-sm cp-device-input" aria-label="读取线圈装置地址" @focus="openDeviceMenu = ''" />
                    <button class="cp-device-toggle" type="button" :aria-expanded="openDeviceMenu === 'readCoil'" aria-label="选择读取线圈装置" @click="toggleDeviceMenu('readCoil')">▾</button>
                    <div v-if="openDeviceMenu === 'readCoil'" class="cp-device-menu">
                      <button v-for="device in deviceAddresses" :key="device.value" type="button" class="cp-device-option" :class="{ selected: readCoilDev === device.value }" @click="selectDevice('readCoil', device.value)">
                        <strong>{{ device.value }}</strong><span>{{ device.label }}</span>
                      </button>
                      <small class="cp-device-hint">可直接输入自定义地址</small>
                    </div>
                  </div>
                </div>
                <div class="cp-field">
                  <label>起始地址</label>
                  <input v-model.number="readCoilStart" type="number" min="0" max="65535" class="input input-sm" />
                </div>
                <div class="cp-field">
                  <label>数量</label>
                  <input v-model.number="readCoilCount" type="number" min="1" class="input input-sm" />
                </div>
              </div>
              <div class="cp-action"><span>读取 {{ readCoilCount || 0 }} 个线圈</span><button class="btn btn-primary" :disabled="writing" @click="terminal.sendReadCoils">发送指令 →</button></div>
              <div v-if="readCoilResp.error" class="msg msg-error">{{ readCoilResp.error }}</div>
            </div>

            <div class="cp-block">
              <div class="cp-block-head">
                <div><small>WRITE COILS</small><span class="cp-block-title">写入线圈</span></div>
                <span class="cp-fc">05 / 15</span>
              </div>
              <div class="cp-fields">
                <div class="cp-field">
                  <label>装置地址</label>
                  <div class="cp-device-picker" @click.stop>
                    <input v-model.number="writeCoilDev" type="number" min="0" max="255" class="input input-sm cp-device-input" aria-label="写入线圈装置地址" @focus="openDeviceMenu = ''" />
                    <button class="cp-device-toggle" type="button" :aria-expanded="openDeviceMenu === 'writeCoil'" aria-label="选择写入线圈装置" @click="toggleDeviceMenu('writeCoil')">▾</button>
                    <div v-if="openDeviceMenu === 'writeCoil'" class="cp-device-menu">
                      <button v-for="device in deviceAddresses" :key="device.value" type="button" class="cp-device-option" :class="{ selected: writeCoilDev === device.value }" @click="selectDevice('writeCoil', device.value)">
                        <strong>{{ device.value }}</strong><span>{{ device.label }}</span>
                      </button>
                      <small class="cp-device-hint">可直接输入自定义地址</small>
                    </div>
                  </div>
                </div>
                <div class="cp-field">
                  <label>起始地址</label>
                  <input v-model.number="writeCoilStart" type="number" min="0" max="65535" class="input input-sm" />
                </div>
                <div class="cp-field cp-field-wide">
                  <label>状态值</label>
                  <input v-model="writeCoilStates" placeholder="1 0 1" class="input input-sm input-mono" />
                </div>
              </div>
              <div class="cp-action"><span>状态使用 1 / 0</span><button class="btn btn-primary" :disabled="writing" @click="terminal.sendWriteCoils">发送指令 →</button></div>
              <div v-if="writeCoilResp.error" class="msg msg-error">{{ writeCoilResp.error }}</div>
            </div>
          </div>

          <div v-show="activeTab === 'ota'" class="cp-section">
            <div class="cp-block cp-ota">
              <div class="cp-block-head">
                <div><small>FIRMWARE DELIVERY</small><span class="cp-block-title">固件更新</span></div>
                <span class="cp-fc">OTA</span>
              </div>
              <div class="cp-file-area" :class="{ selected: otaFileName }">
                <div class="cp-file-mark">{{ otaFileName ? 'BIN' : '+' }}</div>
                <div class="cp-file-info"><strong>{{ otaFileName || '选择 .bin / .ota 固件' }}</strong><small>{{ otaFileName ? '文件已就绪，可检查参数后更新' : '.ota 文件将自动解析并校验文件头' }}</small></div>
                <button class="btn btn-sm btn-outline" :disabled="otaActive" @click="terminal.selectFirmware">{{ otaFileName ? '更换文件' : '选择文件' }}</button>
              </div>

              <div class="cp-config-title"><span>更新参数</span><small v-if="fileKind === 'packaged'">已从文件头填充，可修改</small></div>
              <div class="cp-fields">
                <div class="cp-field">
                  <label>装置地址</label>
                  <div class="cp-device-picker" @click.stop>
                    <input v-model.number="otaDev" type="number" min="0" max="255" class="input input-sm cp-device-input" aria-label="OTA 装置地址" @focus="openDeviceMenu = ''" />
                    <button class="cp-device-toggle" type="button" :aria-expanded="openDeviceMenu === 'ota'" aria-label="选择 OTA 装置" @click="toggleDeviceMenu('ota')">▾</button>
                    <div v-if="openDeviceMenu === 'ota'" class="cp-device-menu">
                      <button v-for="device in deviceAddresses" :key="device.value" type="button" class="cp-device-option" :class="{ selected: otaDev === device.value }" @click="selectDevice('ota', device.value)">
                        <strong>{{ device.value }}</strong><span>{{ device.label }}</span>
                      </button>
                      <small class="cp-device-hint">可直接输入自定义地址</small>
                    </div>
                  </div>
                </div>
                <div class="cp-field">
                  <label>厂家编码</label>
                  <input v-model="otaManufacturer" placeholder="00003339" class="input input-sm input-mono" />
                </div>
                <div class="cp-field">
                  <label>更新次数</label>
                  <input v-model.number="cycles" type="number" min="1" max="100" step="1" class="input input-sm" :disabled="otaActive" />
                </div>
              </div>
              <div class="cp-fields">
                <div class="cp-field">
                  <label>硬件版本</label>
                  <input v-model="otaHwVersion" placeholder="0102" class="input input-sm input-mono" />
                </div>
                <div class="cp-field">
                  <label>软件版本</label>
                  <input v-model="otaSwVersion" placeholder="0304" class="input input-sm input-mono" />
                </div>
              </div>
              <dl v-if="otaInfo" class="ota-meta">
                <div><dt>产品类型</dt><dd>{{ bytesHex(otaInfo.deviceType) }}</dd></div>
                <div><dt>设备地址</dt><dd>{{ otaInfo.deviceAddress ?? '不限制' }}</dd></div>
                <div><dt>文件描述</dt><dd>{{ otaInfo.description || '—' }}</dd></div>
                <div><dt>升级内容</dt><dd>{{ fileSize(otaInfo.upgradeSize) }}</dd></div>
                <div><dt>Upgrade CRC</dt><dd>{{ crcHex(otaInfo.upgradeCrc) }}</dd></div>
                <div><dt>File CRC</dt><dd>{{ crcHex(otaInfo.fileCrc) }}</dd></div>
              </dl>

              <div v-if="otaActive" class="cp-progress">
                <div class="cp-progress-head">
                  <div class="cp-progress-status">
                    <i></i>
                    <span><small>正在更新</small><strong>{{ otaProgress }}%</strong></span>
                  </div>
                  <div class="cp-timer">
                    <small>已用时间</small>
                    <time>{{ duration(otaElapsedSeconds) }}</time>
                  </div>
                  <strong class="cp-progress-pct">{{ otaProgress }}<small>%</small></strong>
                </div>
                <div class="cp-progress-track">
                  <div class="cp-progress-fill" :style="{ width: otaProgress + '%' }"></div>
                </div>
              </div>

              <div class="cp-action cp-ota-action">
                <span>{{ otaFileName ? `${cycles} 次更新，每次间隔 2 秒` : '选择固件后才能开始' }}</span>
                <button v-if="!otaActive" class="btn btn-primary" :disabled="!otaFileName || writing" @click="terminal.startOta">开始更新 →</button>
                <button v-else class="btn btn-danger" :disabled="otaCancelling" @click="terminal.cancelOta">{{ otaCancelling ? '正在终止…' : '终止更新' }}</button>
              </div>
              <div v-if="otaResult" class="cp-result" :class="`cp-result-${otaResult.type}`">
                <span class="cp-result-mark">{{ otaResult.type === 'success' ? '✓' : otaResult.type === 'warn' ? '!' : '×' }}</span>
                <span class="cp-result-copy"><small>更新结果</small><strong>{{ otaResult.message }}</strong></span>
                <span class="cp-result-time"><small>总用时</small><time>{{ duration(otaElapsedSeconds) }}</time></span>
              </div>
              <div v-if="otaError && !otaResult" class="msg msg-error">{{ otaError }}</div>
            </div>
          </div>
        </div>
      </main>

      <aside class="cp-log">
        <div class="cp-log-head">
          <span class="cp-log-title"><i></i>实时通信流 <small>{{ commLog.length }}</small></span>
          <button class="btn btn-xs btn-ghost" @click="terminal.clearLogs">清空</button>
        </div>
        <div class="cp-log-body">
          <div v-if="!commLog.length" class="cp-log-empty"><span>_</span>发送指令后，响应将在这里显示</div>
          <div v-for="entry in commLog" :key="entry.id" class="cp-log-entry" :class="'cp-log-' + entry.type">
            <div class="cp-log-entry-head">
              <span class="cp-log-time">{{ entry.time }}</span>
              <span class="cp-log-tag">{{ entry.label }}</span>
            </div>
            <div v-if="entry.error" class="cp-log-err">{{ entry.error }}</div>
            <template v-else>
              <div v-if="entry.request" class="cp-log-line cp-log-tx"><span class="cp-log-k">TX</span><code>{{ entry.request }}</code></div>
              <div v-if="entry.raw" class="cp-log-line cp-log-rx"><span class="cp-log-k">RX</span><code>{{ entry.raw }}</code></div>
              <div v-if="entry.data" class="cp-log-line cp-log-data"><span class="cp-log-k">DATA</span><code>{{ entry.data }}</code></div>
            </template>
          </div>
        </div>
      </aside>
    </div>
  </section>
</template>

<style scoped>
.comm-head { display: flex; align-items: flex-end; justify-content: space-between; gap: 20px; margin-bottom: 24px; }
.comm-head > div > span { color: var(--primary); font: 700 .62rem/1 'Cascadia Code', monospace; letter-spacing: .14em; }
.comm-head h1 { margin: 8px 0 2px; font-size: clamp(1.75rem, 3vw, 2.5rem); line-height: 1; letter-spacing: -.045em; }
.comm-head p { margin: 0; color: var(--text-muted); font-size: .72rem; }
.terminal-state { display: flex; align-items: center; gap: 7px; color: var(--success); font: 700 .58rem/1 'Cascadia Code', monospace; }
.terminal-state i { width: 6px; height: 6px; border-radius: 50%; background: currentColor; box-shadow: 0 0 8px currentColor; }
.cp-layout {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.cp-main {
  display: flex;
  flex-direction: column;
  gap: 0;
}

.cp-tabs {
  display: flex;
  flex-wrap: wrap;
  gap: 2px;
  padding: 3px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
}

.cp-tab {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 7px 14px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
  font-family: inherit;
  font-size: 0.78rem;
  font-weight: 500;
  white-space: nowrap;
  transition: all var(--transition-fast);
}

.cp-tab:active { transform: scale(0.96); }

.cp-tab.active {
  background: var(--primary);
  color: var(--text-inverse);
  box-shadow: var(--shadow-sm);
}

.cp-tab-icon {
  width: 16px;
  height: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.68rem;
  font-weight: 700;
  opacity: 0.6;
}

.cp-tab.active .cp-tab-icon { opacity: 1; }

.cp-panel { margin-top: 10px; }

.cp-section {
  display: flex;
  flex-direction: column;
  gap: 0;
}

.cp-block {
  padding: 18px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  display: flex;
  flex-direction: column;
  gap: 10px;
  box-shadow: var(--shadow-sm);
  min-width: 0;
}

.cp-block-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.cp-block-title {
  font-size: 1rem;
  font-weight: 700;
  color: var(--text-primary);
}

.cp-fc {
  font-size: 0.65rem;
  font-weight: 600;
  font-family: 'SF Mono', 'Cascadia Code', 'Consolas', monospace;
  color: var(--text-muted);
  color: var(--primary);
  background: var(--primary-light);
  padding: 2px 8px;
  border-radius: var(--radius-xs);
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

.cp-device-picker {
  position: relative;
  min-width: 0;
}

.cp-device-input {
  padding-right: 34px !important;
}

.cp-device-toggle {
  position: absolute;
  top: 3px;
  right: 3px;
  display: grid;
  place-items: center;
  width: 28px;
  height: calc(100% - 6px);
  padding: 0;
  border: 0;
  border-left: 1px solid var(--border);
  border-radius: 0 var(--radius-xs) var(--radius-xs) 0;
  color: var(--primary);
  background: transparent;
  cursor: pointer;
  font: 800 .85rem/1 'Cascadia Code', monospace;
}

.cp-device-toggle:hover,
.cp-device-toggle[aria-expanded="true"] {
  background: var(--primary-light);
}

.cp-device-menu {
  position: absolute;
  z-index: 110;
  top: calc(100% + 6px);
  left: 0;
  width: 100%;
  min-width: 156px;
  max-height: min(360px, calc(100dvh - 170px));
  overflow-y: auto;
  padding: 5px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface);
  box-shadow: var(--shadow-md), 0 8px 24px rgba(10, 32, 44, .12);
}

.cp-device-option {
  display: grid;
  grid-template-columns: 28px 1fr;
  align-items: center;
  width: 100%;
  min-height: 30px;
  padding: 5px 7px;
  border: 0;
  border-radius: 5px;
  color: var(--text-secondary);
  background: transparent;
  cursor: pointer;
  text-align: left;
}

.cp-device-option:hover,
.cp-device-option.selected {
  color: var(--primary);
  background: var(--primary-light);
}

.cp-device-option strong {
  font: 750 .68rem/1 'Cascadia Code', monospace;
}

.cp-device-option span {
  overflow: hidden;
  font-size: .68rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cp-device-hint {
  display: block;
  margin: 4px 7px 2px;
  padding-top: 5px;
  border-top: 1px solid var(--border);
  color: var(--text-muted);
  font-size: .57rem;
}

.input-mono {
  font-family: 'SF Mono', 'Cascadia Code', 'Consolas', monospace !important;
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

.cp-divider {
  height: 1px;
  background: var(--border);
  margin: 12px 0;
}

.cp-file-area {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border: 1px dashed var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--bg);
}

.ota-meta { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 1px; margin: 0; overflow: hidden; border: 1px solid var(--border); border-radius: var(--radius-sm); background: var(--border); }
.ota-meta div { min-width: 0; padding: 8px 10px; background: var(--bg); }
.ota-meta dt { color: var(--text-muted); font-size: .62rem; }
.ota-meta dd { overflow: hidden; margin: 2px 0 0; color: var(--text-primary); font: 600 .7rem/1.35 'Cascadia Code', monospace; text-overflow: ellipsis; white-space: nowrap; }

.cp-file-info {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  min-width: 0;
  color: var(--text-muted);
  font-size: 0.78rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cp-progress { margin: 4px 0; }

.cp-progress-track {
  height: 6px;
  background: var(--border-strong);
  border-radius: 3px;
  overflow: hidden;
  margin-bottom: 6px;
}

.cp-progress-fill {
  height: 100%;
  background: linear-gradient(90deg, var(--primary), #60a5fa);
  border-radius: 3px;
  transition: width 0.25s ease;
}

.cp-progress-pct {
  font-weight: 700;
  color: var(--primary);
  font-family: 'SF Mono', 'Cascadia Code', 'Consolas', monospace;
}

.cp-log {
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: #0b1826;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  max-height: 360px;
  box-shadow: var(--shadow-sm);
}

.cp-log-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  border-bottom: 1px solid var(--border);
  background: rgba(255,255,255,.04);
  border-color: rgba(255,255,255,.08);
}

.cp-log-title {
  font-size: 0.72rem;
  font-weight: 600;
  color: #a9bcc3;
}

.cp-log-body {
  overflow-y: auto;
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.cp-log-empty {
  text-align: center;
  padding: 20px 0;
  font-size: 0.72rem;
  color: var(--text-muted);
}

.cp-log-entry {
  padding: 8px 10px;
  border-radius: var(--radius-sm);
  border: 1px solid rgba(255,255,255,.08);
  background: rgba(255,255,255,.035);
  font-size: 0.72rem;
}

.cp-log-success { border-color: rgba(34, 197, 94, 0.2); }
.cp-log-error { border-color: rgba(239, 68, 68, 0.2); background: rgba(239, 68, 68, 0.02); }

.cp-log-entry-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
}

.cp-log-time {
  font-size: 0.62rem;
  font-family: 'SF Mono', 'Cascadia Code', 'Consolas', monospace;
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
  line-height: 1.45;
}

.cp-log-k {
  font-size: 0.6rem;
  font-weight: 700;
  color: var(--text-muted);
  flex-shrink: 0;
  padding-top: 1px;
  min-width: 28px;
}

.cp-log-line code {
  font-size: 0.72rem;
  font-family: 'SF Mono', 'Cascadia Code', 'Consolas', monospace;
  color: #6bd2cd;
  word-break: break-all;
}

.cp-log-data code { color: var(--success); }
.cp-log-tx code { color: #f1bc72; }
.cp-log-rx code { color: #6bd2cd; }

@media (min-width: 900px) {
  .cp-layout {
    flex-direction: row;
    align-items: flex-start;
    gap: 16px;
  }

  .cp-main {
    flex: 1;
    min-width: 0;
  }

  .cp-section { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 14px; }
  .cp-section:has(.cp-block:only-child) { grid-template-columns: 1fr; }
  .cp-divider { display: none; }

  .cp-log {
    width: 340px;
    flex-shrink: 0;
    max-height: calc(100vh - 180px);
    position: sticky;
    top: 70px;
  }

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
}

/* Protocol bench: mode rail, command canvas, live wire trace. */
.comm-page { width: 100%; overflow: visible; }
.cp-workspace { display: grid; min-width: 0; gap: 12px; }
.cp-canvas { min-width: 0; }
.cp-panel { margin-top: 0; }
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
  transition: color var(--transition-fast), background var(--transition-fast), transform var(--transition-fast);
}
.cp-mode:active { transform: scale(.97); }
.cp-mode.active { color: var(--text-primary); background: var(--surface-hover); box-shadow: var(--shadow-sm); }
.cp-mode-icon { display: grid; place-items: center; width: 24px; height: 24px; flex: 0 0 auto; border: 1px solid var(--border-strong); border-radius: 5px; font: 800 .68rem/1 'Cascadia Code', monospace; }
.cp-mode.active .cp-mode-icon { color: var(--text-inverse); border-color: var(--primary); background: var(--primary); }
.cp-mode-copy { min-width: 0; display: flex; flex-direction: column; }
.cp-mode-copy strong { overflow: hidden; font-size: .7rem; text-overflow: ellipsis; white-space: nowrap; }
.cp-mode-copy small { color: var(--text-muted); font: 600 .5rem/1.4 'Cascadia Code', monospace; white-space: nowrap; }
.cp-section { display: grid; gap: 12px; margin-top: 12px; }
.cp-block { gap: 16px; padding: clamp(16px, 2.2vw, 24px); border-color: var(--border-strong); box-shadow: none; }
.cp-block-head > div { display: flex; flex-direction: column; }
.cp-block-head > div > small { color: var(--primary); font: 700 .52rem/1 'Cascadia Code', monospace; letter-spacing: .1em; }
.cp-block-title { margin-top: 6px; font-size: 1.05rem; letter-spacing: -.025em; }
.cp-fc { min-width: 34px; padding: 7px 8px; color: var(--primary); border: 1px solid color-mix(in srgb, var(--primary) 22%, transparent); border-radius: 6px; text-align: center; }
.cp-action { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding-top: 12px; border-top: 1px solid var(--border); }
.cp-action > span { color: var(--text-muted); font-size: .64rem; }
.cp-action .btn { flex: 0 0 auto; }
.cp-file-area { padding: 14px; border-style: solid; background: var(--bg); }
.cp-file-area.selected { border-color: color-mix(in srgb, var(--success) 40%, var(--border)); background: var(--success-light); }
.cp-file-mark { display: grid; place-items: center; width: 40px; height: 40px; flex: 0 0 auto; border-radius: 7px; color: var(--primary); background: var(--primary-light); font: 800 .58rem/1 'Cascadia Code', monospace; }
.cp-file-area.selected .cp-file-mark { color: var(--success); background: var(--surface); }
.cp-file-info { display: flex; align-items: flex-start; flex: 1 1 auto; flex-direction: column; gap: 1px; min-width: 0; width: 100%; }
.cp-file-info strong { display: block; width: 100%; max-width: 100%; overflow: visible; color: var(--text-primary); font-size: .76rem; line-height: 1.35; overflow-wrap: anywhere; word-break: break-all; white-space: normal; }
.cp-file-info small { color: var(--text-muted); font-size: .6rem; }
.cp-config-title { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding-top: 2px; }
.cp-config-title span { color: var(--text-secondary); font-size: .72rem; font-weight: 700; }
.cp-config-title small { color: var(--success); font-size: .6rem; }
.cp-progress { display: flex; flex-direction: column; gap: 13px; padding: 14px 15px; border: 1px solid color-mix(in srgb, var(--primary) 20%, var(--border)); border-radius: 9px; background: var(--primary-light); }
.cp-progress-head { display: grid; grid-template-columns: minmax(0, 1fr) auto auto; align-items: center; gap: 16px; }
.cp-progress-status { min-width: 0; display: flex; align-items: center; gap: 9px; }
.cp-progress-status > i { width: 7px; height: 7px; flex: 0 0 auto; border-radius: 50%; background: var(--primary); box-shadow: 0 0 0 5px color-mix(in srgb, var(--primary) 12%, transparent); animation: pulse 1.4s ease-in-out infinite; }
.cp-progress-status > span { min-width: 0; display: flex; flex-direction: column; }
.cp-progress-status small, .cp-timer small, .cp-result-copy small, .cp-result-time small { color: var(--text-muted); font-size: .54rem; font-weight: 700; letter-spacing: .06em; }
.cp-progress-status strong { overflow: hidden; color: var(--text-primary); font-size: .7rem; text-overflow: ellipsis; white-space: nowrap; }
.cp-timer { display: flex; align-items: flex-end; flex-direction: column; padding-right: 16px; border-right: 1px solid var(--border-strong); }
.cp-timer time { color: var(--text-primary); font: 800 1.18rem/.95 'Cascadia Code', monospace; font-variant-numeric: tabular-nums; letter-spacing: .04em; }
.cp-progress-pct { min-width: 52px; color: var(--primary); font: 800 1.3rem/1 'Cascadia Code', monospace; text-align: right; }
.cp-progress-pct small { margin-left: 1px; font-size: .56rem; }
.cp-progress-track { height: 5px; margin: 0; background: var(--surface); }
.cp-progress-fill { background: var(--primary); }
.cp-result { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; gap: 11px; padding: 12px 13px; border: 1px solid var(--border); border-radius: 9px; background: var(--bg); }
.cp-result-mark { display: grid; place-items: center; width: 30px; height: 30px; border-radius: 7px; font-size: .82rem; font-weight: 900; }
.cp-result-copy { min-width: 0; display: flex; flex-direction: column; }
.cp-result-copy strong { overflow: visible; color: var(--text-primary); font-size: .7rem; overflow-wrap: anywhere; word-break: break-all; white-space: pre-line; }
.cp-result-time { display: flex; align-items: flex-end; flex-direction: column; padding-left: 14px; border-left: 1px solid var(--border); }
.cp-result-time time { color: var(--text-primary); font: 800 .92rem/1 'Cascadia Code', monospace; font-variant-numeric: tabular-nums; }
.cp-result-success { border-color: color-mix(in srgb, var(--success) 30%, var(--border)); background: var(--success-light); }
.cp-result-success .cp-result-mark { color: var(--success); background: var(--surface); }
.cp-result-warn { border-color: color-mix(in srgb, var(--warning) 30%, var(--border)); background: var(--warning-light); }
.cp-result-warn .cp-result-mark { color: var(--warning); background: var(--surface); }
.cp-result-error { border-color: color-mix(in srgb, var(--danger) 30%, var(--border)); background: var(--danger-light); }
.cp-result-error .cp-result-mark { color: var(--danger); background: var(--surface); }
.cp-log { position: static; width: auto; max-height: 400px; border-color: #203846; border-radius: var(--radius-md); box-shadow: none; }
.cp-log-head { min-height: 46px; padding: 10px 14px; }
.cp-log-title { display: flex; align-items: center; gap: 7px; }
.cp-log-title i { width: 6px; height: 6px; border-radius: 50%; background: #59d0aa; box-shadow: 0 0 8px rgba(89,208,170,.7); }
.cp-log-title small { display: grid; place-items: center; min-width: 18px; height: 18px; padding: 0 4px; border-radius: 5px; color: #6f8994; background: rgba(255,255,255,.05); font: 600 .55rem/1 'Cascadia Code', monospace; }
.cp-log-body { min-height: 170px; padding: 10px; }
.cp-log-empty { display: flex; align-items: center; justify-content: center; flex-direction: column; gap: 7px; min-height: 140px; color: #58707b; }
.cp-log-empty span { color: #65cfc8; font: 700 1.1rem/1 'Cascadia Code', monospace; animation: pulse 1.2s steps(2) infinite; }
.cp-log-entry { border-left-width: 3px; border-radius: 5px; }
.cp-log-line { margin-top: 3px; }
.cp-log-k { min-width: 34px; }

@media (min-width: 760px) {
  .cp-section { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .cp-section:has(.cp-block:only-child) { grid-template-columns: 1fr; }
  .cp-ota { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .cp-ota > .cp-block-head, .cp-ota > .cp-file-area, .cp-ota > .cp-config-title, .cp-ota > .ota-meta, .cp-ota > .cp-progress, .cp-ota > .cp-ota-action, .cp-ota > .cp-result, .cp-ota > .msg { grid-column: 1 / -1; }
}

@media (min-width: 1180px) {
  .cp-workspace { grid-template-columns: 112px minmax(0, 1fr) 330px; align-items: start; gap: 14px; }
  .cp-modes { position: sticky; top: 70px; grid-template-columns: 1fr; padding: 5px; }
  .cp-mode { justify-content: flex-start; padding: 11px 9px; }
  .cp-section { margin-top: 0; }
  .cp-log { position: sticky; top: 70px; max-height: calc(100vh - 110px); }
}

@media (max-width: 540px) {
  .comm-head p, .cp-mode-copy small { display: none; }
  .comm-head { align-items: center; margin-bottom: 16px; }
  .terminal-state { font-size: 0; }
  .terminal-state i { width: 8px; height: 8px; }
  .cp-mode { gap: 5px; padding: 8px 5px; }
  .cp-mode-icon { width: 21px; height: 21px; }
  .cp-mode-copy strong { font-size: .62rem; }
  .cp-action { align-items: stretch; flex-direction: column; }
  .cp-action .btn { width: 100%; }
  .cp-file-area { align-items: flex-start; flex-wrap: wrap; }
  .cp-file-area .btn { width: 100%; }
  .ota-meta { grid-template-columns: 1fr; }
  .cp-progress-head { gap: 9px; }
  .cp-progress-pct { min-width: 42px; font-size: 1rem; }
  .cp-timer { padding-right: 0; border-right: 0; }
  .cp-result { gap: 8px; }
  .cp-result-mark { width: 27px; height: 27px; }
  .cp-result-copy strong { white-space: normal; }
}

@media (max-width: 767px) {
  .comm-head { display: none; }
}
</style>
