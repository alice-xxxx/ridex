<script setup>
import { computed, onMounted, onUnmounted, ref, watch } from "vue"
import { storeToRefs } from "pinia"
import { useSessionStore } from "../stores/session.js"

const session = useSessionStore()
const {
  endpoints, scanning, scanError, connecting, connectingEndpoint, connectError,
  disconnecting,
  accessMode, networkPlatform, networkUsername, networkPassword,
  serialSupported,
} = storeToRefs(session)
const networkName = ref("")
const serialBaud = ref(115200)
const filterKeyword = ref("")
const deviceFilterEnabled = ref(true)
const activitySeconds = ref(0)
let activityClock

const namedDevices = computed(() => {
  const keyword = filterKeyword.value.trim().toLowerCase()
  return endpoints.value
    .filter((device) => device.transport === "ble")
    .filter((device) => {
      const name = device.name?.trim()
      if (!name || name === device.id) return false
      const normalized = name.toLowerCase()
      return (!deviceFilterEnabled.value || normalized.startsWith("g") || normalized.startsWith("1"))
        && (!keyword || normalized.includes(keyword))
    })
    .map((device) => ({ address: device.id, name: device.name, rssi: device.signal }))
})
const serialDevices = computed(() => endpoints.value
  .filter((device) => device.transport === "serial")
  .map((device) => ({ path: device.id, name: device.name })))
const networkReady = computed(() => networkName.value.trim().length === 16
)
watch([scanning, connecting], ([isScanning, isConnecting]) => {
  clearInterval(activityClock)
  activitySeconds.value = 0
  if (isScanning || isConnecting) {
    activityClock = setInterval(() => { activitySeconds.value += 1 }, 1000)
  }
}, { immediate: true })
watch(accessMode, (mode) => {
  if (mode === "serial" && serialSupported.value && !serialDevices.value.length) session.discover("serial")
})
onMounted(async () => {
  await session.loadChannelCaps()
  await session.loadNetworkCredentials()
})
onUnmounted(() => clearInterval(activityClock))
</script>

<template>
  <section class="page search-page">
    <header class="page-head">
      <div>
        <h1>{{ accessMode === 'ble' ? '附近设备' : accessMode === 'network' ? '网络连接' : '串口连接' }}</h1>
      </div>
      <button v-if="accessMode === 'ble'" class="btn btn-primary" :disabled="scanning || connecting || disconnecting" @click="session.discover('ble')">
        {{ scanning ? `扫描中 ${activitySeconds}s` : '扫描设备' }}
      </button>
      <button v-if="accessMode === 'serial'" class="btn btn-primary" :disabled="scanning || connecting || disconnecting" @click="session.discover('serial')">
        {{ scanning ? '正在读取' : '刷新串口' }}
      </button>
    </header>

    <nav class="access-modes" :class="{ 'without-serial': !serialSupported }" aria-label="连接方式">
      <button :class="{ active: accessMode === 'ble' }" @click="accessMode = 'ble'"><span>BLE</span><strong>附近蓝牙</strong><small>直接连接车辆</small></button>
      <button :class="{ active: accessMode === 'network' }" @click="accessMode = 'network'"><span>NET</span><strong>网络通道</strong><small>通过 Garow 服务</small></button>
      <button v-if="serialSupported" :class="{ active: accessMode === 'serial' }" @click="accessMode = 'serial'"><span>COM</span><strong>串口链路</strong><small>USB / UART 直连</small></button>
    </nav>

    <div v-if="accessMode === 'ble'" class="filter-bar">
      <label class="filter-box">
        <svg viewBox="0 0 16 16" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.6"><circle cx="7" cy="7" r="4.5"/><path d="M10.5 10.5L14 14"/></svg>
        <input v-model="filterKeyword" placeholder="筛选设备名称" aria-label="筛选设备名称" />
        <code>{{ namedDevices.length }}</code>
      </label>
      <label class="device-filter">
        <span><strong>设备过滤</strong><small>仅显示 G / 1 开头</small></span>
        <input v-model="deviceFilterEnabled" type="checkbox" role="switch" aria-label="仅显示以 G 或 1 开头的蓝牙设备" />
      </label>
    </div>

    <div v-if="disconnecting" class="msg msg-warn">正在断开设备连接，请稍候。</div>
    <div v-if="scanError || connectError" class="msg msg-error">{{ scanError || connectError }}</div>
    <div v-if="accessMode === 'ble' && namedDevices.length" class="device-grid">
      <button
        v-for="device in namedDevices"
        :key="device.address"
        class="device-card"
        :disabled="connecting || disconnecting"
        @click="session.connect({ transport: 'ble', address: device.address, name: device.name, timeoutMs: 8000 })"
      >
        <span class="device-icon">BLE</span>
        <span class="device-main"><strong>{{ device.name }}</strong><code>{{ device.address }}</code></span>
        <span v-if="device.rssi" class="rssi" :class="session.rssiLevel(device.rssi)">{{ device.rssi }}<small>dBm</small></span>
        <span class="connect-arrow">{{ connectingEndpoint === device.address ? `${activitySeconds}s` : '→' }}</span>
      </button>
    </div>

    <div v-else-if="accessMode === 'ble' && !scanning" class="empty-state">
      <strong>尚未发现设备</strong>
      <p>确认车辆蓝牙已开启，然后开始扫描。</p>
    </div>

    <div v-if="accessMode === 'serial'" class="filter-bar serial-options">
      <label class="filter-box"><span>波特率</span><select v-model.number="serialBaud" class="input input-mono"><option v-for="baud in [9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600]" :key="baud" :value="baud">{{ baud }}</option></select></label>
      <span>8 数据位 · 无校验 · 1 停止位 · 无流控</span>
    </div>

    <div v-if="accessMode === 'serial' && serialDevices.length" class="device-grid">
      <button v-for="device in serialDevices" :key="device.path" class="device-card" :disabled="connecting || disconnecting" @click="session.connect({ transport: 'serial', path: device.path, baudRate: serialBaud })">
        <span class="device-icon">COM</span>
        <span class="device-main"><strong>{{ device.name }}</strong><code>{{ device.path }} · {{ serialBaud }} baud</code></span>
        <span class="connect-arrow">{{ connectingEndpoint === device.path ? `${activitySeconds}s` : '→' }}</span>
      </button>
    </div>

    <div v-else-if="accessMode === 'serial' && !scanning" class="empty-state">
      <strong>没有可用串口</strong><p>连接 USB 串口设备后点击刷新串口。</p>
    </div>

    <form v-if="accessMode === 'network'" class="network-connect" @submit.prevent="session.connect({ transport: 'network', name: networkName, platform: networkPlatform, username: networkUsername, password: networkPassword, timeoutMs: 10000 })">
      <div class="network-mark"><span></span><i></i><b></b></div>
      <div class="network-copy"><span>GAROW REMOTE LINK</span><h2>输入车辆SN名称</h2><p>通过服务器转发 Modbus 指令，无需车辆在电脑或手机附近。</p></div>
      <div class="network-fields">
        <div class="platform-switch" role="radiogroup" aria-label="网络平台">
          <button type="button" :class="{ active: networkPlatform === 'production' }" role="radio" :aria-checked="networkPlatform === 'production'" @click="session.selectNetworkPlatform('production')"><i></i><span>正式平台</span><small>PRODUCTION</small></button>
          <button type="button" :class="{ active: networkPlatform === 'test' }" role="radio" :aria-checked="networkPlatform === 'test'" @click="session.selectNetworkPlatform('test')"><i></i><span>测试平台</span><small>DEV IOT</small></button>
        </div>
        <label><span>SN名称</span><input v-model.trim="networkName" class="input input-mono" :placeholder="networkPlatform === 'test' ? '例如 G50A23264TA9PTW2' : '例如 G66A24254LA99PYB'" maxlength="16" autocomplete="off" spellcheck="false" autofocus /></label>
        <label><span>账号</span><input v-model.trim="networkUsername" class="input input-mono" autocomplete="username" spellcheck="false" /></label>
        <label><span>密码</span><input v-model="networkPassword" type="password" class="input input-mono" autocomplete="current-password" /></label>
      </div>
      <button class="btn btn-primary" :disabled="connecting || disconnecting || !networkReady">{{ connecting ? `正在连接 ${activitySeconds}s` : '连接车辆 →' }}</button>
      <small>网络连接使用服务端安全会话，车辆名称必须与平台登记名称完全一致。</small>
    </form>
  </section>
</template>

<style scoped>
.search-page { width: 100%; }
.page-head { display: flex; align-items: end; justify-content: space-between; gap: 16px; margin-bottom: 18px; }
.page-head span { color: var(--primary); font: 700 .62rem/1 'Cascadia Code', monospace; letter-spacing: .14em; }
.page-head h1 { margin: 6px 0 0; font-size: clamp(1.8rem, 4vw, 2.6rem); letter-spacing: -.05em; }
.access-modes { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 7px; margin-bottom: 12px; padding: 4px; border: 1px solid var(--border); border-radius: 11px; background: var(--surface); }
.access-modes.without-serial { grid-template-columns: repeat(2, minmax(0, 1fr)); }
.access-modes button { display: grid; grid-template-columns: auto 1fr; grid-template-rows: auto auto; column-gap: 9px; padding: 10px 11px; border: 0; border-radius: 8px; color: var(--text-muted); background: transparent; text-align: left; cursor: pointer; }
.access-modes button > span { grid-row: 1 / 3; align-self: center; display: grid; place-items: center; width: 34px; height: 34px; border: 1px solid var(--border); border-radius: 7px; font: 800 .57rem/1 'Cascadia Code', monospace; }
.access-modes strong { color: var(--text-secondary); font-size: .72rem; }
.access-modes small { font-size: .57rem; }
.access-modes button.active { color: var(--primary); background: var(--primary-light); box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--primary) 18%, transparent); }
.access-modes button.active > span { color: var(--text-inverse); border-color: var(--primary); background: var(--primary); }
.access-modes button.active strong { color: var(--text-primary); }
.filter-bar { display: flex; gap: 8px; margin-bottom: 12px; }
.filter-box { min-width: 0; flex: 1; display: flex; align-items: center; gap: 9px; padding: 11px 13px; border: 1px solid var(--border); border-radius: 9px; color: var(--text-muted); background: var(--surface); }
.filter-box input { min-width: 0; flex: 1; border: 0; outline: 0; color: var(--text-primary); background: transparent; font-size: .82rem; }
.filter-box code { font-size: .6rem; }
.device-filter { display: flex; align-items: center; gap: 12px; padding: 7px 11px 7px 13px; border: 1px solid var(--border); border-radius: 9px; background: var(--surface); cursor: pointer; }
.device-filter span { display: flex; flex-direction: column; white-space: nowrap; }
.device-filter strong { color: var(--text-secondary); font-size: .68rem; }
.device-filter small { margin-top: 2px; color: var(--text-muted); font-size: .56rem; }
.device-filter input { width: 34px; height: 18px; margin: 0; accent-color: var(--primary); cursor: pointer; }
.serial-options { align-items: center; padding: 10px 12px; border: 1px solid var(--border); border-radius: 9px; background: var(--surface); }
.serial-options .filter-box { flex: 0 1 280px; padding: 0; border: 0; }
.serial-options .filter-box > span, .serial-options > span { color: var(--text-muted); font-size: .62rem; }
.serial-options select { min-width: 150px; }
.device-grid { display: grid; gap: 8px; }
.device-card { width: 100%; display: flex; align-items: center; gap: 12px; padding: 13px; border: 1px solid var(--border); border-radius: 9px; color: inherit; background: var(--surface); text-align: left; cursor: pointer; transition: .15s ease; }
.device-card:hover { border-color: var(--border-strong); transform: translateY(-1px); box-shadow: var(--shadow-md); }
.device-card:disabled { opacity: .65; cursor: wait; }
.device-icon { display: grid; place-items: center; width: 38px; height: 38px; flex: 0 0 auto; border-radius: 7px; color: var(--primary); background: var(--primary-light); font: 800 .58rem/1 'Cascadia Code', monospace; }
.device-main { min-width: 0; flex: 1; display: flex; flex-direction: column; }
.device-main strong { overflow: hidden; font-size: .82rem; text-overflow: ellipsis; white-space: nowrap; }
.device-main code { overflow: hidden; color: var(--text-muted); font-size: .6rem; text-overflow: ellipsis; white-space: nowrap; }
.rssi { flex: 0 0 auto; color: var(--text-secondary); font: 700 .68rem/1 'Cascadia Code', monospace; white-space: nowrap; }
.rssi small { margin-left: 2px; color: var(--text-muted); font-size: .48rem; }
.rssi.excellent { color: var(--success); } .rssi.good { color: var(--primary); } .rssi.fair { color: var(--warning); } .rssi.weak { color: var(--danger); }
.connect-arrow { display: grid; place-items: center; width: 30px; height: 30px; flex: 0 0 30px; border-radius: 6px; color: var(--primary); background: var(--primary-light); }
.empty-state { display: grid; min-height: 220px; place-items: center; align-content: center; padding: 24px; color: var(--text-muted); text-align: center; }
.empty-state strong { color: var(--text-secondary); font-size: .9rem; }
.empty-state p { margin: 5px 0 0; font-size: .72rem; }
.network-connect { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; gap: 18px; padding: clamp(18px, 3vw, 30px); border: 1px solid var(--border-strong); border-radius: 12px; background: var(--surface); box-shadow: var(--shadow-sm); }
.network-mark { position: relative; width: 58px; height: 58px; display: grid; place-items: center; border-radius: 12px; background: var(--primary-light); }
.network-mark span, .network-mark i, .network-mark b { position: absolute; height: 2px; border-radius: 2px; background: var(--primary); transform-origin: left center; }
.network-mark span { width: 28px; transform: translate(-9px, 8px) rotate(-48deg); }
.network-mark i { width: 24px; transform: translate(5px, -9px) rotate(45deg); }
.network-mark b { width: 7px; height: 7px; border-radius: 50%; box-shadow: -24px 17px 0 var(--primary), 18px 17px 0 var(--primary); }
.network-copy span { color: var(--primary); font: 700 .52rem/1 'Cascadia Code', monospace; letter-spacing: .11em; }
.network-copy h2 { margin: 6px 0 2px; font-size: 1rem; }
.network-copy p { margin: 0; color: var(--text-muted); font-size: .65rem; }
.network-fields { grid-column: 2; display: grid; gap: 10px; }
.platform-switch { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 5px; padding: 4px; border: 1px solid var(--border); border-radius: 9px; background: var(--bg); }
.platform-switch button { min-width: 0; display: grid; grid-template-columns: auto 1fr auto; align-items: center; gap: 7px; padding: 8px 9px; border: 0; border-radius: 6px; color: var(--text-muted); background: transparent; text-align: left; cursor: pointer; }
.platform-switch button > i { width: 7px; height: 7px; border-radius: 50%; background: var(--text-muted); box-shadow: 0 0 0 3px color-mix(in srgb, currentColor 10%, transparent); }
.platform-switch button > span { color: var(--text-secondary); font-size: .67rem; font-weight: 750; }
.platform-switch button > small { font: 700 .48rem/1 'Cascadia Code', monospace; letter-spacing: .06em; }
.platform-switch button.active { color: var(--primary); background: var(--surface); box-shadow: var(--shadow-sm); }
.platform-switch button.active > i { background: var(--primary); }
.platform-switch button.active > span { color: var(--text-primary); }
.network-connect label { display: flex; flex-direction: column; gap: 5px; }
.network-fields label { grid-column: auto; }
.network-connect label > span { color: var(--text-secondary); font-size: .66rem; font-weight: 700; }
.network-connect > button { grid-column: 3; grid-row: 2; align-self: end; min-height: 40px; }
.network-connect > small { grid-column: 2 / -1; color: var(--text-muted); font-size: .57rem; }
@media (min-width: 760px) { .device-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
@media (max-width: 620px) { .page-head { align-items: center; } .filter-bar { align-items: stretch; flex-direction: column; } .device-filter { justify-content: space-between; } .device-card { gap: 8px; padding-inline: 10px; } .device-icon { width: 34px; height: 34px; } .rssi small { display: none; } .network-connect { grid-template-columns: auto 1fr; gap: 13px; } .network-fields, .network-connect > button, .network-connect > small { grid-column: 1 / -1; } .network-connect > button { grid-row: auto; width: 100%; } .platform-switch button { grid-template-columns: auto 1fr; } .platform-switch button > small { display: none; } }
</style>
