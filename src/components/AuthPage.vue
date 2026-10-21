<script setup lang="ts">
import { useSessionStore } from "../stores/session"
import DeviceDisconnectButton from "./DeviceDisconnectButton.vue"

const session = useSessionStore()
const { link, authorization } = session
</script>

<template>
  <section class="page auth-page">
    <div class="auth-context">
      <div class="device-plate">
        <div class="device-symbol" aria-hidden="true">
          <svg viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" stroke-width="1.6">
            <path d="M7 7h10v10H7zM10 2v5m4-5v5m-4 10v5m4-5v5M2 10h5m-5 4h5m10-4h5m-5 4h5" />
          </svg>
        </div>
        <div class="device-meta">
          <span>已连接设备</span>
          <strong>{{ link.current?.name || "未命名设备" }}</strong>
          <code>{{ link.current?.endpoint }}</code>
        </div>
        <DeviceDisconnectButton class="device-disconnect" :disabled="link.disconnecting" @click="session.disconnect" />
      </div>
    </div>

    <div class="auth-panel">
      <form @submit.prevent="session.authenticate()" class="auth-form">
        <label>
          <span>车辆代码 <small>可留空直接尝试</small></span>
          <input v-model="authorization.vehicleCode" placeholder="输入车辆代码" class="input input-mono" autocomplete="off"
            spellcheck="false" />
        </label>

        <p v-if="link.connectError || authorization.error" class="msg msg-error">
          {{ link.connectError || authorization.error }}
        </p>

        <button type="submit" class="btn btn-primary btn-block auth-submit" :disabled="authorization.active">
          <span v-if="authorization.active" class="spinner"></span>
          {{ authorization.active ? "正在协商会话..." : "认证并读取车辆" }}
          <span v-if="!authorization.active" aria-hidden="true">→</span>
        </button>
        <button type="button" class="blank-auth" :disabled="authorization.active" @click="session.authenticate('')">
          使用默认认证
        </button>
      </form>
    </div>
  </section>
</template>

<style scoped>
.auth-page {
  width: 100%;
  min-width: 0;
  min-height: calc(100dvh - 4rem);
  display: grid;
  align-items: center;
  gap: 2.625rem;
  max-width: 65.625rem;
  margin: 0 auto;
  overflow-x: visible;
}

.auth-context {
  min-width: 0;
  max-width: 35rem;
}

.device-plate {
  display: flex;
  align-items: center;
  gap: 0.875rem;
  padding: 1rem;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-lg);
  background: var(--surface);
  box-shadow: var(--shadow-md);
}

.device-symbol {
  display: grid;
  place-items: center;
  width: 2.875rem;
  height: 2.875rem;
  flex: 0 0 auto;
  border-radius: 0.5625rem;
  color: var(--primary);
  background: var(--primary-light);
}

.device-meta {
  min-width: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
}

.device-meta span {
  color: var(--text-muted);
  font:
    700 0.55rem/1.5 "Cascadia Code",
    monospace;
  letter-spacing: 0.1em;
}

.device-meta strong {
  overflow: hidden;
  font-size: 0.9rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.device-meta code {
  overflow: hidden;
  color: var(--text-muted);
  font-size: 0.65rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.device-disconnect {
  display: grid;
  place-items: center;
  width: 2.125rem;
  height: 2.125rem;
  flex: 0 0 auto;
  border: 0;
  border-radius: 0.4375rem;
  color: var(--danger);
  background: var(--danger-light);
  cursor: pointer;
}

.device-disconnect:disabled {
  opacity: 0.5;
  cursor: wait;
}

.auth-panel {
  min-width: 0;
  width: 100%;
  padding: 1.5rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-xl);
  background: var(--surface);
  box-shadow: 0 16px 50px rgba(11, 24, 38, 0.1);
}

.auth-form {
  display: flex;
  flex-direction: column;
  gap: 1.0625rem;
}

.auth-form label {
  display: flex;
  flex-direction: column;
  gap: 0.4375rem;
}

.auth-form label>span {
  display: flex;
  justify-content: space-between;
  color: var(--text-secondary);
  font-size: 0.72rem;
  font-weight: 700;
}

.auth-form label small {
  color: var(--text-muted);
  font-weight: 500;
}

.auth-submit {
  min-height: 2.8125rem;
  justify-content: space-between;
  margin-top: 0.25rem;
}

.blank-auth {
  border: 0;
  color: var(--text-muted);
  background: transparent;
  font-size: 0.72rem;
  cursor: pointer;
}

.blank-auth:hover {
  color: var(--primary);
}

@media (min-width: 900px) {
  .auth-page {
    grid-template-columns: minmax(0, 1.35fr) minmax(20rem, 0.8fr);
  }
}

@media (max-width: 899px) {
  .auth-page {
    min-height: auto;
    align-items: start;
    gap: 1.5rem;
    padding: 0.75rem 0 max(2rem, env(safe-area-inset-bottom));
  }

  .auth-panel {
    padding: 1.25rem;
  }

  .auth-form input {
    scroll-margin-top: calc(4.5rem + env(safe-area-inset-top));
    scroll-margin-bottom: 35vh;
  }
}
</style>
