<script setup lang="ts">
import { computed } from "vue"
import { useSessionStore } from "../stores/session"

const session = useSessionStore()
const { link } = session
const connectionStatus = computed(() => {
  if (session.authenticated) {
    return { text: "安全会话已建立", code: `${session.transport.toUpperCase()} / READY` }
  }
  if (link.current) {
    return { text: "设备已连接", code: `${session.transport.toUpperCase()} / LINKED` }
  }
  return { text: "等待设备接入", code: "ACCESS / STANDBY" }
})
</script>

<template>
  <aside class="shell-nav">
    <div class="brand">
      <span class="brand-mark" aria-hidden="true"><i></i><b>RX</b></span>
      <span class="brand-copy"><strong>RideX</strong><small>VEHICLE LAB</small></span>
    </div>

    <div class="session-state" :class="{ connected: link.current, authenticated: session.authenticated }">
      <span class="state-line"><i></i>{{ connectionStatus.text }}</span>
      <span class="state-code">{{ connectionStatus.code }}</span>
    </div>

    <button
      class="mobile-ota-link"
      :class="{ active: session.page === 'ota-tool' }"
      :aria-current="session.page === 'ota-tool' ? 'page' : undefined"
      aria-label="固件转换"
      title="固件转换"
      @click="session.goTo('ota-tool')"
    >
      <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.8">
        <path d="M12 3v11" />
        <path d="m8 10 4 4 4-4" />
        <path d="M5 17v3a1 1 0 001 1h12a1 1 0 001-1v-3" />
      </svg>
    </button>

    <nav class="primary-nav" aria-label="主导航">
      <button
        :class="{ active: session.page === 'search' }"
        :disabled="session.page !== 'search' && !(session.page === 'ota-tool' && !link.current)"
        @click="session.goTo('search')"
      >
        <span class="nav-index">LINK</span><span>设备接入<small>BLE / Network</small></span>
      </button>
      <button
        :class="{ active: session.page === 'auth' }"
        :disabled="!link.current || session.authenticated"
        @click="session.goTo('auth')"
      >
        <span class="nav-index">KEY</span><span>安全认证<small>Authorize</small></span>
      </button>
      <button
        :class="{ active: session.page === 'status' }"
        :disabled="!session.authenticated"
        @click="session.goTo('status')"
      >
        <span class="nav-index">DAT</span><span>车辆状态<small>Telemetry</small></span>
      </button>
      <button
        :class="{ active: session.page === 'terminal' }"
        :disabled="!session.authenticated"
        @click="session.goTo('terminal')"
      >
        <span class="nav-index">I/O</span><span>协议终端<small>Modbus / OTA</small></span>
      </button>
      <button :class="{ active: session.page === 'ota-tool' }" @click="session.goTo('ota-tool')">
        <span class="nav-index">OTA</span><span>固件工具<small>多格式 → OTA</small></span>
      </button>
    </nav>

    <div v-if="link.current" class="device-dock">
      <div class="device-copy">
        <span>当前设备</span>
        <strong>{{ link.current.name || "未命名设备" }}</strong>
        <code>{{ link.current.endpoint }}</code>
      </div>
      <button
        :disabled="link.disconnecting"
        @click="session.disconnect"
        title="断开连接"
        aria-label="断开设备连接"
      >
        <span v-if="link.disconnecting" class="spinner-sm"></span>
        <svg
          v-else
          viewBox="0 0 20 20"
          width="17"
          height="17"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
        >
          <path
            d="M7 3v3M13 3v3M3 8h14M5 5h10a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2V7a2 2 0 012-2z"
          />
        </svg>
      </button>
    </div>
  </aside>
</template>

<style scoped>
.shell-nav {
  position: fixed;
  inset: 0 auto 0 0;
  z-index: 100;
  width: 236px;
  display: flex;
  flex-direction: column;
  padding: 22px 16px 16px;
  color: #dbe8e9;
  background: #0b1826;
  border-right: 1px solid rgba(139, 224, 219, 0.13);
}

.brand {
  display: flex;
  align-items: center;
  gap: 11px;
  padding: 0 7px 22px;
}
.brand-mark {
  position: relative;
  display: grid;
  place-items: center;
  width: 38px;
  height: 34px;
  color: #67d4cf;
}
.brand-mark::before,
.brand-mark::after {
  content: "";
  position: absolute;
  inset: 0;
  border: 1px solid currentColor;
  border-radius: 6px;
  transform: skewX(-8deg);
}
.brand-mark::after {
  inset: 5px -3px -3px 5px;
  opacity: 0.22;
}
.brand-mark b {
  font:
    800 0.72rem/1 "Cascadia Code",
    monospace;
  letter-spacing: 0.08em;
}
.brand-mark i {
  position: absolute;
  right: -1px;
  top: 3px;
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: #f09a4b;
  box-shadow: 0 0 8px #f09a4b;
}
.brand-copy {
  display: flex;
  flex-direction: column;
}
.brand-copy strong {
  font-size: 1.05rem;
  letter-spacing: -0.025em;
}
.brand-copy small {
  color: #68808b;
  font:
    700 0.56rem/1.4 "Cascadia Code",
    monospace;
  letter-spacing: 0.15em;
}

.mobile-ota-link {
  display: none;
}

.session-state {
  padding: 13px 12px;
  margin-bottom: 18px;
  border: 1px solid rgba(255, 255, 255, 0.07);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.025);
}
.state-line {
  display: flex;
  align-items: center;
  gap: 7px;
  font-size: 0.72rem;
  font-weight: 650;
  color: #8396a0;
}
.state-line i {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #53636c;
}
.state-code {
  display: block;
  margin: 5px 0 0 14px;
  color: #4f626c;
  font:
    600 0.52rem/1 "Cascadia Code",
    monospace;
  letter-spacing: 0.1em;
}
.session-state.connected .state-line {
  color: #77c9c5;
}
.session-state.connected .state-line i {
  background: #40c1ba;
  box-shadow: 0 0 10px rgba(64, 193, 186, 0.8);
}
.session-state.authenticated {
  border-color: rgba(64, 193, 186, 0.2);
  background: rgba(64, 193, 186, 0.055);
}

.primary-nav {
  display: flex;
  flex-direction: column;
  gap: 5px;
}
.primary-nav button {
  display: grid;
  grid-template-columns: 28px 1fr;
  align-items: center;
  width: 100%;
  padding: 11px 10px;
  border: 0;
  border-radius: 8px;
  color: #81939d;
  background: transparent;
  text-align: left;
  cursor: pointer;
  transition: 0.16s ease;
}
.primary-nav button:not(:disabled):hover {
  color: #dbe8e9;
  background: rgba(255, 255, 255, 0.04);
}
.primary-nav button.active {
  color: #e8ffff;
  background: rgba(0, 143, 149, 0.18);
  box-shadow: inset 3px 0 #44c8c1;
}
.primary-nav button:disabled {
  opacity: 0.34;
  cursor: default;
}
.primary-nav button > span:last-child {
  display: flex;
  flex-direction: column;
  gap: 1px;
  font-size: 0.8rem;
  font-weight: 650;
}
.primary-nav small {
  color: #5d717b;
  font:
    500 0.55rem/1.3 "Cascadia Code",
    monospace;
  letter-spacing: 0.04em;
}
.nav-index {
  color: #4f626c;
  font:
    600 0.58rem/1 "Cascadia Code",
    monospace;
}
.primary-nav button.active .nav-index,
.primary-nav button.active small {
  color: #63aaa8;
}

.device-dock {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: auto;
  padding: 12px;
  border-radius: 9px;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.07);
}
.device-copy {
  min-width: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
}
.device-copy span {
  color: #617680;
  font-size: 0.58rem;
}
.device-copy strong {
  overflow: hidden;
  color: #dbe8e9;
  font-size: 0.73rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.device-copy code {
  overflow: hidden;
  color: #6e858f;
  font-size: 0.55rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.device-dock button {
  display: grid;
  place-items: center;
  width: 32px;
  height: 32px;
  flex: 0 0 auto;
  border: 0;
  border-radius: 6px;
  color: #df7466;
  background: rgba(216, 79, 62, 0.1);
  cursor: pointer;
}
.device-dock button:disabled {
  opacity: 0.5;
}
.spinner-sm {
  width: 14px;
  height: 14px;
  border: 2px solid transparent;
  border-top-color: currentColor;
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
}

@media (max-width: 767px) {
  .shell-nav {
    position: sticky;
    inset: 0 0 auto;
    width: 100%;
    height: calc(58px + env(safe-area-inset-top));
    flex-direction: row;
    align-items: center;
    padding: calc(8px + env(safe-area-inset-top)) 12px 8px;
  }
  .brand {
    padding: 0;
  }
  .brand-mark {
    width: 32px;
    height: 28px;
  }
  .brand-copy small {
    display: none;
  }
  .mobile-ota-link {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    flex: 0 0 auto;
    margin-left: 7px;
    padding: 0;
    border: 1px solid rgba(103, 212, 207, 0.24);
    border-radius: 6px;
    color: #8bd8d4;
    background: rgba(64, 193, 186, 0.08);
    cursor: pointer;
  }
  .mobile-ota-link.active {
    color: #e8ffff;
    border-color: rgba(103, 212, 207, 0.55);
    background: rgba(64, 193, 186, 0.24);
  }
  .session-state {
    margin: 0 0 0 auto;
    padding: 7px 9px;
  }
  .state-code,
  .primary-nav,
  .device-copy {
    display: none;
  }
  .device-dock {
    margin: 0 0 0 7px;
    padding: 0;
    border: 0;
    background: transparent;
  }
  .device-dock button {
    width: 34px;
    height: 34px;
  }
}
</style>
