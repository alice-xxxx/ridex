<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, watch } from "vue"
import { useSessionStore } from "../stores/session"
import type { NetworkPlatform } from "../types"

const session = useSessionStore()
const { access, link } = session
const form = reactive({
  network: {
    name: "G50A23264TA9PTW2",
    platform: "production" as NetworkPlatform,
    username: "",
    password: "",
    saved: false,
  },
  serialBaud: 115200,
  filterKeyword: "",
  deviceFilterEnabled: true,
  activitySeconds: 0,
})
let activityClock: ReturnType<typeof setInterval> | undefined
let autoScanTimer: ReturnType<typeof setTimeout> | undefined
const autoScanReady = computed(() =>
  access.mode === "ble" && session.page === "search" && !link.current &&
  !access.scanning && !link.connecting && !link.disconnecting && !access.error,
)
const namedDevices = computed(() => {
  const keyword = form.filterKeyword.trim().toLowerCase()
  return access.endpoints
    .filter((device) => device.transport === "ble")
    .filter((device) => {
      const name = device.name.trim()
      if (!name || name === device.id) {
        return device.systemConnected && (!keyword || device.id.toLowerCase().includes(keyword))
      }
      const normalized = name.toLowerCase()
      return (
        (!form.deviceFilterEnabled || normalized.startsWith("g") || normalized.startsWith("1")) &&
        (!keyword || normalized.includes(keyword))
      )
    })
})
const serialDevices = computed(() =>
  access.endpoints.filter((device) => device.transport === "serial"),
)
watch(autoScanReady, (ready) => {
  clearTimeout(autoScanTimer)
  if (ready && !document.hidden) autoScanTimer = setTimeout(scanBle, 3000)
})
watch(
  [() => access.scanning, () => link.connecting],
  ([isScanning, isConnecting]) => {
    clearInterval(activityClock)
    form.activitySeconds = 0
    if (isScanning || isConnecting) {
      activityClock = setInterval(() => {
        form.activitySeconds += 1
      }, 1000)
    }
  },
  { immediate: true },
)
watch(
  () => access.mode,
  (mode) => {
    if (mode === "ble") scanBle()
    else if (mode === "serial" && access.serialSupported && !serialDevices.value.length) {
      session.discover("serial")
    }
  },
)
onMounted(async () => {
  session.loadSerialSupport()
  document.addEventListener("visibilitychange", scanBle)
  scanBle()
  await selectNetworkPlatform(form.network.platform)
})
onUnmounted(() => {
  clearInterval(activityClock)
  clearTimeout(autoScanTimer)
  document.removeEventListener("visibilitychange", scanBle)
})

function scanBle() {
  clearTimeout(autoScanTimer)
  if (autoScanReady.value && !document.hidden) void session.discover("ble")
}

function rssiLevel(rssi: number) {
  if (rssi >= -40) return "excellent"
  if (rssi >= -60) return "good"
  if (rssi >= -80) return "fair"
  return "weak"
}

async function selectNetworkPlatform(platform: NetworkPlatform) {
  Object.assign(form.network, { platform, username: "", password: "", saved: false })
  link.connectError = ""
  const credentials = await session.loadNetworkCredentials(platform)
  if (form.network.platform === platform) Object.assign(form.network, credentials)
}

function connectNetwork() {
  const request = {
    transport: "network" as const,
    name: form.network.name,
    platform: form.network.platform,
    username: form.network.username,
    password: form.network.password,
    timeoutMs: 10000,
  }
  if (form.network.saved) {
    request.username = ""
    request.password = ""
  }
  return session.connect(request)
}
</script>

<template>
  <section class="page search-page">
    <nav class="access-modes" :class="{ 'without-serial': !access.serialSupported }" aria-label="连接方式">
      <button :class="{ active: access.mode === 'ble' }" @click="access.mode = 'ble'">
        <span>BLE</span>
      </button>
      <button :class="{ active: access.mode === 'network' }" @click="access.mode = 'network'">
        <span>NET</span>
      </button>
      <button v-if="access.serialSupported" :class="{ active: access.mode === 'serial' }"
        @click="access.mode = 'serial'">
        <span>COM</span>
      </button>
      <button type="button" class="ota-tool-button" aria-label="固件工具" title="固件工具" @click="session.goTo('ota-tool')">
        <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.8"
          aria-hidden="true">
          <path d="M12 3v11" />
          <path d="m8 10 4 4 4-4" />
          <path d="M5 17v3a1 1 0 001 1h12a1 1 0 001-1v-3" />
        </svg>
      </button>
    </nav>

    <div v-if="access.mode === 'ble'" class="ble-controls">
      <label class="filter-box">
        <svg viewBox="0 0 16 16" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.6">
          <circle cx="7" cy="7" r="4.5" />
          <path d="M10.5 10.5L14 14" />
        </svg>
        <input v-model="form.filterKeyword" placeholder="筛选设备名称" aria-label="筛选设备名称" />
      </label>
      <label class="device-filter" title="仅显示名称以 G 或 1 开头的蓝牙设备">
        <strong>G / 1</strong>
        <input v-model="form.deviceFilterEnabled" type="checkbox" role="switch" aria-label="仅显示以 G 或 1 开头的蓝牙设备" />
      </label>
      <button class="btn btn-primary ble-scan-button"
        :disabled="access.scanning || link.connecting || link.disconnecting" @click="session.discover('ble')">
        {{ access.scanning ? `扫描中 ${form.activitySeconds}s` : "扫描设备" }}
      </button>
    </div>

    <div v-if="link.disconnecting" class="msg msg-warn">正在断开设备连接，请稍候。</div>
    <div v-if="access.error || link.connectError" class="msg msg-error">
      {{ access.error || link.connectError }}
    </div>
    <div v-if="access.mode === 'ble' && namedDevices.length" class="device-grid">
      <button v-for="device in namedDevices" :key="device.id" class="device-card"
        :disabled="link.connecting || link.disconnecting" @click="
          session.connect({
            transport: 'ble',
            address: device.id,
            name: device.name,
            timeoutMs: 8000,
          })
          ">
        <span class="device-main"><strong>{{ device.name && device.name !== device.id ? device.name : 'BLE 设备' }}</strong><code>{{ device.id }}{{ device.systemConnected ? ' · 系统已连接' : '' }}</code></span>
        <span v-if="device.signal !== null" class="rssi" :class="rssiLevel(device.signal)">{{ device.signal
        }}<small>dBm</small></span>
        <span class="connect-arrow">{{
          link.endpoint === device.id ? `${form.activitySeconds}s` : "→"
        }}</span>
      </button>
    </div>

    <div v-else-if="access.mode === 'ble' && !access.scanning" class="empty-state">
      <strong>尚未发现设备</strong>
    </div>

    <div v-if="access.mode === 'serial'" class="serial-options">
      <label class="filter-box"><span>波特率</span><select v-model.number="form.serialBaud" class="input input-mono">
          <option v-for="baud in [9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600]" :key="baud" :value="baud">
            {{ baud }}
          </option>
        </select></label>
      <button class="btn btn-primary serial-refresh-button"
        :disabled="access.scanning || link.connecting || link.disconnecting" @click="session.discover('serial')">
        {{ access.scanning ? "正在读取" : "刷新串口" }}
      </button>
    </div>

    <div v-if="access.mode === 'serial' && serialDevices.length" class="device-grid">
      <button v-for="device in serialDevices" :key="device.id" class="device-card"
        :disabled="link.connecting || link.disconnecting"
        @click="session.connect({ transport: 'serial', path: device.id, baudRate: form.serialBaud })">
        <span class="device-main"><strong>{{ device.name }}</strong></span>
        <span class="connect-arrow">{{
          link.endpoint === device.id ? `${form.activitySeconds}s` : "→"
        }}</span>
      </button>
    </div>

    <div v-else-if="access.mode === 'serial' && !access.scanning" class="empty-state">
      <strong>没有可用串口</strong>
      <p>连接 USB 串口设备后点击刷新串口。</p>
    </div>

    <form v-if="access.mode === 'network'" class="network-connect" @submit.prevent="connectNetwork">
      <div class="network-fields">
        <div class="platform-switch" role="radiogroup" aria-label="网络平台">
          <button type="button" :class="{ active: form.network.platform === 'production' }" role="radio"
            :aria-checked="form.network.platform === 'production'" @click="selectNetworkPlatform('production')">
            <i></i><span>正式平台</span>
          </button>
          <button type="button" :class="{ active: form.network.platform === 'test' }" role="radio"
            :aria-checked="form.network.platform === 'test'" @click="selectNetworkPlatform('test')">
            <i></i><span>测试平台</span>
          </button>
        </div>
        <label><span>SN</span><input v-model.trim="form.network.name" class="input input-mono" maxlength="16"
            autocomplete="off" spellcheck="false" /></label>
        <label><span>账号</span><input v-model.trim="form.network.username" class="input input-mono"
            autocomplete="username" spellcheck="false" @input="form.network.saved = false" /></label>
        <label><span>密码</span><input v-model="form.network.password" type="password" class="input input-mono"
            autocomplete="current-password" :placeholder="form.network.saved ? '已保存' : ''"
            @input="form.network.saved = false" /></label>
      </div>
      <button class="btn btn-primary" :disabled="link.connecting || link.disconnecting">
        {{ link.connecting ? `正在连接 ${form.activitySeconds}s` : "连接车辆 →" }}
      </button>
    </form>
  </section>
</template>

<style scoped>
.search-page {
  width: 100%;
}

.access-modes {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr)) 2.625rem;
  gap: 0.4375rem;
  min-width: 0;
  margin-bottom: 0.75rem;
  padding: 0.25rem;
  border: 1px solid var(--border);
  border-radius: 0.6875rem;
  background: var(--surface);
}

.access-modes.without-serial {
  grid-template-columns: repeat(2, minmax(0, 1fr)) 2.625rem;
}

.access-modes button {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 2.625rem;
  padding: 0.625rem 0.6875rem;
  border: 0;
  border-radius: 0.5rem;
  color: var(--text-muted);
  background: transparent;
  font-size: 0.72rem;
  font-weight: 700;
  letter-spacing: 0.02em;
  cursor: pointer;
}

.access-modes button.active {
  color: var(--primary);
  background: var(--primary-light);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--primary) 18%, transparent);
}

.access-modes .ota-tool-button {
  display: grid;
  place-items: center;
  width: 2.25rem;
  min-height: 2.25rem;
  padding: 0;
  justify-self: center;
  border-radius: 0.5rem;
  color: var(--primary);
  background: transparent;
}

.access-modes .ota-tool-button:hover {
  background: var(--primary-light);
}

.ble-controls {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto;
  align-items: center;
  gap: 0;
  margin-bottom: 0.75rem;
  padding: 0.25rem;
  border: 1px solid var(--border);
  border-radius: 0.6875rem;
  background: var(--surface);
}

.filter-box {
  min-width: 0;
  flex: 1;
  display: flex;
  align-items: center;
  gap: 0.5625rem;
  padding: 0.6875rem 0.8125rem;
  border: 1px solid var(--border);
  border-radius: 0.5625rem;
  color: var(--text-muted);
  background: var(--surface);
}

.filter-box input {
  min-width: 0;
  flex: 1;
  border: 0;
  outline: 0;
  color: var(--text-primary);
  background: transparent;
  font-size: 0.82rem;
}

.ble-controls .filter-box {
  padding: 0.5rem 0.625rem;
  border: 0;
  border-right: 1px solid var(--border);
  border-radius: 0;
  background: transparent;
}

.device-filter {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.4375rem 0.6875rem 0.4375rem 0.8125rem;
  border: 1px solid var(--border);
  border-radius: 0.5625rem;
  background: var(--surface);
  cursor: pointer;
}

.ble-controls .device-filter {
  gap: 0.375rem;
  padding: 0.4375rem 0.625rem;
  border: 0;
  border-radius: 0;
  background: transparent;
  white-space: nowrap;
}

.ble-scan-button {
  min-height: 2.5rem;
  margin-left: 0.25rem;
  padding-inline: 0.875rem;
  white-space: nowrap;
}

.device-filter strong {
  color: var(--text-secondary);
  font-size: 0.68rem;
}

.device-filter input {
  width: 2.125rem;
  height: 1.125rem;
  margin: 0;
  accent-color: var(--primary);
  cursor: pointer;
}

.serial-options {
  display: flex;
  margin-bottom: 0.75rem;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem;
  border: 1px solid var(--border);
  border-radius: 0.5625rem;
  background: var(--surface);
}

.serial-options .filter-box {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  align-items: center;
  gap: 0.5rem;
  flex: 1 1 17.5rem;
  min-width: 0;
  padding: 0;
  border: 0;
  background: transparent;
}

.serial-options .filter-box>span {
  color: var(--text-muted);
  font-size: 0.62rem;
  white-space: nowrap;
}

.serial-options select {
  min-width: 0;
  min-height: 2.5rem;
  padding: 0.4375rem 0.625rem;
}

.serial-refresh-button {
  flex: 0 0 auto;
}

.device-grid {
  display: grid;
  gap: 0.5rem;
}

.device-card {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.8125rem;
  border: 1px solid var(--border);
  border-radius: 0.5625rem;
  color: inherit;
  background: var(--surface);
  text-align: left;
  cursor: pointer;
  transition: 0.15s ease;
}

.device-card:hover {
  border-color: var(--border-strong);
  transform: translateY(-1px);
  box-shadow: var(--shadow-md);
}

.device-card:disabled {
  opacity: 0.65;
  cursor: wait;
}

.device-main {
  min-width: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
  -webkit-user-select: text;
  user-select: text;
}

.device-main * {
  -webkit-user-select: text;
  user-select: text;
}

.device-main strong {
  overflow: hidden;
  font-size: 0.82rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.device-main code {
  overflow: hidden;
  color: var(--text-muted);
  font-size: 0.6rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rssi {
  flex: 0 0 auto;
  color: var(--text-secondary);
  font:
    700 0.68rem/1 "Cascadia Code",
    monospace;
  white-space: nowrap;
}

.rssi small {
  margin-left: 0.125rem;
  color: var(--text-muted);
  font-size: 0.48rem;
}

.rssi.excellent {
  color: var(--success);
}

.rssi.good {
  color: var(--primary);
}

.rssi.fair {
  color: var(--warning);
}

.rssi.weak {
  color: var(--danger);
}

.connect-arrow {
  display: grid;
  place-items: center;
  width: 1.875rem;
  height: 1.875rem;
  flex: 0 0 1.875rem;
  border-radius: 0.375rem;
  color: var(--primary);
  background: var(--primary-light);
}

.empty-state {
  display: grid;
  min-height: 13.75rem;
  place-items: center;
  align-content: center;
  padding: 1.5rem;
  color: var(--text-muted);
  text-align: center;
}

.empty-state strong {
  color: var(--text-secondary);
  font-size: 0.9rem;
}

.empty-state p {
  margin: 0.3125rem 0 0;
  font-size: 0.72rem;
}

.network-connect {
  display: grid;
  gap: 0.875rem;
  padding: clamp(1.125rem, 3vw, 1.875rem);
  border: 1px solid var(--border-strong);
  border-radius: 0.75rem;
  background: var(--surface);
  box-shadow: var(--shadow-sm);
}

.network-fields {
  display: grid;
  gap: 0.625rem;
}

.platform-switch {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.3125rem;
  padding: 0.25rem;
  border: 1px solid var(--border);
  border-radius: 0.5625rem;
  background: var(--bg);
}

.platform-switch button {
  min-width: 0;
  display: grid;
  grid-template-columns: auto 1fr;
  align-items: center;
  gap: 0.4375rem;
  padding: 0.5rem 0.5625rem;
  border: 0;
  border-radius: 0.375rem;
  color: var(--text-muted);
  background: transparent;
  text-align: left;
  cursor: pointer;
}

.platform-switch button>i {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--text-muted);
  box-shadow: 0 0 0 3px color-mix(in srgb, currentColor 10%, transparent);
}

.platform-switch button>span {
  color: var(--text-secondary);
  font-size: 0.67rem;
  font-weight: 750;
}

.platform-switch button.active {
  color: var(--primary);
  background: var(--surface);
  box-shadow: var(--shadow-sm);
}

.platform-switch button.active>i {
  background: var(--primary);
}

.platform-switch button.active>span {
  color: var(--text-primary);
}

.network-connect label {
  display: flex;
  flex-direction: column;
  gap: 0.3125rem;
}

.network-connect label>span {
  color: var(--text-secondary);
  font-size: 0.66rem;
  font-weight: 700;
}

.network-connect>button {
  min-height: 2.5rem;
}

.network-connect>small {
  color: var(--text-muted);
  font-size: 0.57rem;
}

@media (min-width: 760px) {
  .device-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 620px) {
  .ble-controls {
    padding: 0.1875rem;
  }

  .ble-controls .filter-box {
    padding-inline: 0.375rem;
  }

  .ble-controls .device-filter {
    gap: 0.25rem;
    padding-inline: 0.4375rem;
  }

  .ble-scan-button {
    padding-inline: 0.625rem;
  }

  .device-filter {
    justify-content: space-between;
  }

  .device-card {
    gap: 0.5rem;
    padding-inline: 0.625rem;
  }

  .rssi small {
    display: none;
  }

  .network-connect {
    gap: 0.75rem;
  }
}

@media (max-width: 420px) {
  .ble-controls {
    padding: 0.125rem;
  }

  .ble-controls .filter-box {
    gap: 0.3125rem;
    padding-inline: 0.25rem;
  }

  .ble-controls .device-filter {
    gap: 0.25rem;
    padding-inline: 0.25rem;
  }

  .device-filter strong {
    font-size: 0.6rem;
  }

  .ble-scan-button {
    margin-left: 0.125rem;
    padding-inline: 0.4375rem;
    font-size: 0.72rem;
  }
}
</style>
