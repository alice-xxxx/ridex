<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, watch } from "vue"
import { useSessionStore } from "../stores/session"
import type { NetworkPlatform } from "../types"

const session = useSessionStore()
const { access, link } = session
const form = reactive({
  network: {
    name: "",
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
const accessTitles = {
  ble: "附近设备",
  network: "网络连接",
  serial: "串口连接",
}

const namedDevices = computed(() => {
  const keyword = form.filterKeyword.trim().toLowerCase()
  return access.endpoints
    .filter((device) => device.transport === "ble")
    .filter((device) => {
      const name = device.name.trim()
      if (!name || name === device.id) return false
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
    if (mode === "serial" && access.serialSupported && !serialDevices.value.length) {
      session.discover("serial")
    }
  },
)
onMounted(async () => {
  session.loadSerialSupport()
  await selectNetworkPlatform(form.network.platform)
})
onUnmounted(() => clearInterval(activityClock))

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
    <header class="page-head">
      <div>
        <h1>{{ accessTitles[access.mode] }}</h1>
      </div>
      <button
        v-if="access.mode === 'ble'"
        class="btn btn-primary"
        :disabled="access.scanning || link.connecting || link.disconnecting"
        @click="session.discover('ble')"
      >
        {{ access.scanning ? `扫描中 ${form.activitySeconds}s` : "扫描设备" }}
      </button>
      <button
        v-if="access.mode === 'serial'"
        class="btn btn-primary"
        :disabled="access.scanning || link.connecting || link.disconnecting"
        @click="session.discover('serial')"
      >
        {{ access.scanning ? "正在读取" : "刷新串口" }}
      </button>
    </header>

    <nav class="access-modes" :class="{ 'without-serial': !access.serialSupported }" aria-label="连接方式">
      <button :class="{ active: access.mode === 'ble' }" @click="access.mode = 'ble'">
        <span>BLE</span><strong>附近蓝牙</strong><small>直接连接车辆</small>
      </button>
      <button :class="{ active: access.mode === 'network' }" @click="access.mode = 'network'">
        <span>NET</span><strong>网络通道</strong><small>通过 Garow 服务</small>
      </button>
      <button
        v-if="access.serialSupported"
        :class="{ active: access.mode === 'serial' }"
        @click="access.mode = 'serial'"
      >
        <span>COM</span><strong>串口链路</strong><small>USB / UART 直连</small>
      </button>
    </nav>

    <div v-if="access.mode === 'ble'" class="filter-bar">
      <label class="filter-box">
        <svg
          viewBox="0 0 16 16"
          width="15"
          height="15"
          fill="none"
          stroke="currentColor"
          stroke-width="1.6"
        >
          <circle cx="7" cy="7" r="4.5" />
          <path d="M10.5 10.5L14 14" />
        </svg>
        <input v-model="form.filterKeyword" placeholder="筛选设备名称" aria-label="筛选设备名称" />
        <code>{{ namedDevices.length }}</code>
      </label>
      <label class="device-filter">
        <span><strong>设备过滤</strong><small>仅显示 G / 1 开头</small></span>
        <input
          v-model="form.deviceFilterEnabled"
          type="checkbox"
          role="switch"
          aria-label="仅显示以 G 或 1 开头的蓝牙设备"
        />
      </label>
    </div>

    <div v-if="link.disconnecting" class="msg msg-warn">正在断开设备连接，请稍候。</div>
    <div v-if="access.error || link.connectError" class="msg msg-error">
      {{ access.error || link.connectError }}
    </div>
    <div v-if="access.mode === 'ble' && namedDevices.length" class="device-grid">
      <button
        v-for="device in namedDevices"
        :key="device.id"
        class="device-card"
        :disabled="link.connecting || link.disconnecting"
        @click="
          session.connect({
            transport: 'ble',
            address: device.id,
            name: device.name,
            timeoutMs: 8000,
          })
        "
      >
        <span class="device-icon">BLE</span>
        <span class="device-main"
          ><strong>{{ device.name }}</strong
          ><code>{{ device.id }}</code></span
        >
        <span
          v-if="device.signal !== null"
          class="rssi"
          :class="rssiLevel(device.signal)"
          >{{ device.signal }}<small>dBm</small></span
        >
        <span class="connect-arrow">{{
          link.endpoint === device.id ? `${form.activitySeconds}s` : "→"
        }}</span>
      </button>
    </div>

    <div v-else-if="access.mode === 'ble' && !access.scanning" class="empty-state">
      <strong>尚未发现设备</strong>
      <p>确认车辆蓝牙已开启，然后开始扫描。</p>
    </div>

    <div v-if="access.mode === 'serial'" class="filter-bar serial-options">
      <label class="filter-box"
        ><span>波特率</span
        ><select v-model.number="form.serialBaud" class="input input-mono">
          <option
            v-for="baud in [9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600]"
            :key="baud"
            :value="baud"
          >
            {{ baud }}
          </option>
        </select></label
      >
      <span>8 数据位 · 无校验 · 1 停止位 · 无流控</span>
    </div>

    <div v-if="access.mode === 'serial' && serialDevices.length" class="device-grid">
      <button
        v-for="device in serialDevices"
        :key="device.id"
        class="device-card"
        :disabled="link.connecting || link.disconnecting"
        @click="session.connect({ transport: 'serial', path: device.id, baudRate: form.serialBaud })"
      >
        <span class="device-icon">COM</span>
        <span class="device-main"
          ><strong>{{ device.name }}</strong
          ><code>{{ device.id }} · {{ form.serialBaud }} baud</code></span
        >
        <span class="connect-arrow">{{
          link.endpoint === device.id ? `${form.activitySeconds}s` : "→"
        }}</span>
      </button>
    </div>

    <div v-else-if="access.mode === 'serial' && !access.scanning" class="empty-state">
      <strong>没有可用串口</strong>
      <p>连接 USB 串口设备后点击刷新串口。</p>
    </div>

    <form
      v-if="access.mode === 'network'"
      class="network-connect"
      @submit.prevent="connectNetwork"
    >
      <div class="network-mark"><span></span><i></i><b></b></div>
      <div class="network-copy">
        <span>GAROW REMOTE LINK</span>
        <h2>输入车辆SN名称</h2>
        <p>通过服务器转发 Modbus 指令，无需车辆在电脑或手机附近。</p>
      </div>
      <div class="network-fields">
        <div class="platform-switch" role="radiogroup" aria-label="网络平台">
          <button
            type="button"
            :class="{ active: form.network.platform === 'production' }"
            role="radio"
            :aria-checked="form.network.platform === 'production'"
            @click="selectNetworkPlatform('production')"
          >
            <i></i><span>正式平台</span><small>PRODUCTION</small>
          </button>
          <button
            type="button"
            :class="{ active: form.network.platform === 'test' }"
            role="radio"
            :aria-checked="form.network.platform === 'test'"
            @click="selectNetworkPlatform('test')"
          >
            <i></i><span>测试平台</span><small>DEV IOT</small>
          </button>
        </div>
        <label
          ><span>SN名称</span
          ><input
            v-model.trim="form.network.name"
            class="input input-mono"
            :placeholder="
              form.network.platform === 'test'
                ? '例如 G50A23264TA9PTW2'
                : '例如 G66A24254LA99PYB'
            "
            maxlength="16"
            autocomplete="off"
            spellcheck="false"
            autofocus
        /></label>
        <label
          ><span>账号</span
          ><input
            v-model.trim="form.network.username"
            class="input input-mono"
            autocomplete="username"
            spellcheck="false"
            @input="form.network.saved = false"
        /></label>
        <label
          ><span>密码</span
          ><input
            v-model="form.network.password"
            type="password"
            class="input input-mono"
            autocomplete="current-password"
            :placeholder="form.network.saved ? '已保存，留空则继续使用' : ''"
            @input="form.network.saved = false"
        /></label>
      </div>
      <button class="btn btn-primary" :disabled="link.connecting || link.disconnecting">
        {{ link.connecting ? `正在连接 ${form.activitySeconds}s` : "连接车辆 →" }}
      </button>
      <small>网络连接使用服务端安全会话，车辆名称必须与平台登记名称完全一致。</small>
    </form>
  </section>
</template>

<style scoped>
.search-page {
  width: 100%;
}
.page-head {
  display: flex;
  align-items: end;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 18px;
}
.page-head span {
  color: var(--primary);
  font:
    700 0.62rem/1 "Cascadia Code",
    monospace;
  letter-spacing: 0.14em;
}
.page-head h1 {
  margin: 6px 0 0;
  font-size: clamp(1.8rem, 4vw, 2.6rem);
  letter-spacing: -0.05em;
}
.access-modes {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 7px;
  margin-bottom: 12px;
  padding: 4px;
  border: 1px solid var(--border);
  border-radius: 11px;
  background: var(--surface);
}
.access-modes.without-serial {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}
.access-modes button {
  display: grid;
  grid-template-columns: auto 1fr;
  grid-template-rows: auto auto;
  column-gap: 9px;
  padding: 10px 11px;
  border: 0;
  border-radius: 8px;
  color: var(--text-muted);
  background: transparent;
  text-align: left;
  cursor: pointer;
}
.access-modes button > span {
  grid-row: 1 / 3;
  align-self: center;
  display: grid;
  place-items: center;
  width: 34px;
  height: 34px;
  border: 1px solid var(--border);
  border-radius: 7px;
  font:
    800 0.57rem/1 "Cascadia Code",
    monospace;
}
.access-modes strong {
  color: var(--text-secondary);
  font-size: 0.72rem;
}
.access-modes small {
  font-size: 0.57rem;
}
.access-modes button.active {
  color: var(--primary);
  background: var(--primary-light);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--primary) 18%, transparent);
}
.access-modes button.active > span {
  color: var(--text-inverse);
  border-color: var(--primary);
  background: var(--primary);
}
.access-modes button.active strong {
  color: var(--text-primary);
}
.filter-bar {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;
}
.filter-box {
  min-width: 0;
  flex: 1;
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 11px 13px;
  border: 1px solid var(--border);
  border-radius: 9px;
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
.filter-box code {
  font-size: 0.6rem;
}
.device-filter {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 7px 11px 7px 13px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--surface);
  cursor: pointer;
}
.device-filter span {
  display: flex;
  flex-direction: column;
  white-space: nowrap;
}
.device-filter strong {
  color: var(--text-secondary);
  font-size: 0.68rem;
}
.device-filter small {
  margin-top: 2px;
  color: var(--text-muted);
  font-size: 0.56rem;
}
.device-filter input {
  width: 34px;
  height: 18px;
  margin: 0;
  accent-color: var(--primary);
  cursor: pointer;
}
.serial-options {
  align-items: center;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--surface);
}
.serial-options .filter-box {
  flex: 0 1 280px;
  padding: 0;
  border: 0;
}
.serial-options .filter-box > span,
.serial-options > span {
  color: var(--text-muted);
  font-size: 0.62rem;
}
.serial-options select {
  min-width: 150px;
}
.device-grid {
  display: grid;
  gap: 8px;
}
.device-card {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 13px;
  border: 1px solid var(--border);
  border-radius: 9px;
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
.device-icon {
  display: grid;
  place-items: center;
  width: 38px;
  height: 38px;
  flex: 0 0 auto;
  border-radius: 7px;
  color: var(--primary);
  background: var(--primary-light);
  font:
    800 0.58rem/1 "Cascadia Code",
    monospace;
}
.device-main {
  min-width: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
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
  margin-left: 2px;
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
  width: 30px;
  height: 30px;
  flex: 0 0 30px;
  border-radius: 6px;
  color: var(--primary);
  background: var(--primary-light);
}
.empty-state {
  display: grid;
  min-height: 220px;
  place-items: center;
  align-content: center;
  padding: 24px;
  color: var(--text-muted);
  text-align: center;
}
.empty-state strong {
  color: var(--text-secondary);
  font-size: 0.9rem;
}
.empty-state p {
  margin: 5px 0 0;
  font-size: 0.72rem;
}
.network-connect {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 18px;
  padding: clamp(18px, 3vw, 30px);
  border: 1px solid var(--border-strong);
  border-radius: 12px;
  background: var(--surface);
  box-shadow: var(--shadow-sm);
}
.network-mark {
  position: relative;
  width: 58px;
  height: 58px;
  display: grid;
  place-items: center;
  border-radius: 12px;
  background: var(--primary-light);
}
.network-mark span,
.network-mark i,
.network-mark b {
  position: absolute;
  height: 2px;
  border-radius: 2px;
  background: var(--primary);
  transform-origin: left center;
}
.network-mark span {
  width: 28px;
  transform: translate(-9px, 8px) rotate(-48deg);
}
.network-mark i {
  width: 24px;
  transform: translate(5px, -9px) rotate(45deg);
}
.network-mark b {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  box-shadow:
    -24px 17px 0 var(--primary),
    18px 17px 0 var(--primary);
}
.network-copy span {
  color: var(--primary);
  font:
    700 0.52rem/1 "Cascadia Code",
    monospace;
  letter-spacing: 0.11em;
}
.network-copy h2 {
  margin: 6px 0 2px;
  font-size: 1rem;
}
.network-copy p {
  margin: 0;
  color: var(--text-muted);
  font-size: 0.65rem;
}
.network-fields {
  grid-column: 2;
  display: grid;
  gap: 10px;
}
.platform-switch {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 5px;
  padding: 4px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--bg);
}
.platform-switch button {
  min-width: 0;
  display: grid;
  grid-template-columns: auto 1fr auto;
  align-items: center;
  gap: 7px;
  padding: 8px 9px;
  border: 0;
  border-radius: 6px;
  color: var(--text-muted);
  background: transparent;
  text-align: left;
  cursor: pointer;
}
.platform-switch button > i {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--text-muted);
  box-shadow: 0 0 0 3px color-mix(in srgb, currentColor 10%, transparent);
}
.platform-switch button > span {
  color: var(--text-secondary);
  font-size: 0.67rem;
  font-weight: 750;
}
.platform-switch button > small {
  font:
    700 0.48rem/1 "Cascadia Code",
    monospace;
  letter-spacing: 0.06em;
}
.platform-switch button.active {
  color: var(--primary);
  background: var(--surface);
  box-shadow: var(--shadow-sm);
}
.platform-switch button.active > i {
  background: var(--primary);
}
.platform-switch button.active > span {
  color: var(--text-primary);
}
.network-connect label {
  display: flex;
  flex-direction: column;
  gap: 5px;
}
.network-fields label {
  grid-column: auto;
}
.network-connect label > span {
  color: var(--text-secondary);
  font-size: 0.66rem;
  font-weight: 700;
}
.network-connect > button {
  grid-column: 3;
  grid-row: 2;
  align-self: end;
  min-height: 40px;
}
.network-connect > small {
  grid-column: 2 / -1;
  color: var(--text-muted);
  font-size: 0.57rem;
}
@media (min-width: 760px) {
  .device-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
@media (max-width: 620px) {
  .page-head {
    align-items: center;
  }
  .filter-bar {
    align-items: stretch;
    flex-direction: column;
  }
  .device-filter {
    justify-content: space-between;
  }
  .device-card {
    gap: 8px;
    padding-inline: 10px;
  }
  .device-icon {
    width: 34px;
    height: 34px;
  }
  .rssi small {
    display: none;
  }
  .network-connect {
    grid-template-columns: auto 1fr;
    gap: 13px;
  }
  .network-fields,
  .network-connect > button,
  .network-connect > small {
    grid-column: 1 / -1;
  }
  .network-connect > button {
    grid-row: auto;
    width: 100%;
  }
  .platform-switch button {
    grid-template-columns: auto 1fr;
  }
  .platform-switch button > small {
    display: none;
  }
}
</style>
