<script setup lang="ts">
import { useSessionStore } from "../stores/session"

const session = useSessionStore()
const { link, authorization } = session
</script>

<template>
  <section class="page auth-page">
    <div class="auth-context">
      <span class="context-index">SESSION / AUTHORIZE</span>
      <h1>建立安全会话</h1>
      <p>认证用于派生本次连接的临时密钥。密钥仅保留在当前 RideX 会话中。</p>

      <div class="device-plate">
        <div class="device-symbol" aria-hidden="true">
          <svg
            viewBox="0 0 24 24"
            width="24"
            height="24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.6"
          >
            <path d="M7 7h10v10H7zM10 2v5m4-5v5m-4 10v5m4-5v5M2 10h5m-5 4h5m10-4h5m-5 4h5" />
          </svg>
        </div>
        <div class="device-meta">
          <span>LINKED DEVICE</span>
          <strong>{{ link.current?.name || "未命名设备" }}</strong>
          <code>{{ link.current?.endpoint }}</code>
        </div>
        <span class="link-light"><i></i>ONLINE</span>
      </div>

      <div class="session-steps" aria-label="认证过程">
        <span><b>01</b>设备已连接</span><i></i><span class="current"><b>02</b>请求认证</span><i></i
        ><span><b>03</b>读取车辆</span>
      </div>
    </div>

    <div class="auth-panel">
      <div class="panel-head">
        <span>SESSION AUTH</span>
        <i></i>
      </div>
      <form @submit.prevent="session.authenticate()" class="auth-form">
        <label>
          <span>BLE 设备名</span>
          <input :value="link.current?.name" class="input input-mono" readonly />
        </label>
        <label>
          <span>车辆代码 <small>可留空直接尝试</small></span>
          <input
            v-model="authorization.vehicleCode"
            placeholder="输入车辆代码"
            class="input input-mono"
            autocomplete="off"
            spellcheck="false"
            autofocus
          />
        </label>

        <p v-if="link.connectError || authorization.error" class="msg msg-error">
          {{ link.connectError || authorization.error }}
        </p>

        <button
          type="submit"
          class="btn btn-primary btn-block auth-submit"
          :disabled="authorization.active"
        >
          <span v-if="authorization.active" class="spinner"></span>
          {{ authorization.active ? "正在协商会话..." : "认证并读取车辆" }}
          <span v-if="!authorization.active" aria-hidden="true">→</span>
        </button>
        <button
          type="button"
          class="blank-auth"
          :disabled="authorization.active"
          @click="session.authenticate('')"
        >
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
  min-height: calc(100dvh - 64px);
  display: grid;
  align-items: center;
  gap: 42px;
  max-width: 1050px;
  margin: 0 auto;
  overflow-x: visible;
}
.auth-context {
  min-width: 0;
  max-width: 560px;
}
.context-index,
.panel-head span {
  color: var(--primary);
  font:
    700 0.64rem/1 "Cascadia Code",
    monospace;
  letter-spacing: 0.14em;
}
.auth-context h1 {
  margin: 12px 0 10px;
  font-size: clamp(2rem, 5vw, 3.5rem);
  line-height: 1.02;
  letter-spacing: -0.055em;
}
.auth-context > p {
  max-width: 470px;
  margin: 0;
  color: var(--text-secondary);
  font-size: 0.9rem;
  line-height: 1.75;
}
.device-plate {
  display: flex;
  align-items: center;
  gap: 14px;
  margin-top: 34px;
  padding: 16px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-lg);
  background: var(--surface);
  box-shadow: var(--shadow-md);
}
.device-symbol {
  display: grid;
  place-items: center;
  width: 46px;
  height: 46px;
  flex: 0 0 auto;
  border-radius: 9px;
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
.link-light {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--success);
  font:
    700 0.58rem/1 "Cascadia Code",
    monospace;
}
.link-light i {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: currentColor;
  box-shadow: 0 0 9px currentColor;
}
.session-steps {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 18px;
  color: var(--text-muted);
  font-size: 0.62rem;
}
.session-steps span {
  display: flex;
  align-items: center;
  gap: 4px;
  white-space: nowrap;
}
.session-steps b {
  font-family: "Cascadia Code", monospace;
}
.session-steps > i {
  height: 1px;
  flex: 1;
  background: var(--border-strong);
}
.session-steps .current {
  color: var(--primary);
}
.auth-panel {
  min-width: 0;
  width: 100%;
  padding: 24px;
  border: 1px solid var(--border);
  border-radius: var(--radius-xl);
  background: var(--surface);
  box-shadow: 0 16px 50px rgba(11, 24, 38, 0.1);
}
.panel-head {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 25px;
}
.panel-head i {
  height: 1px;
  flex: 1;
  background: var(--border);
}
.auth-form {
  display: flex;
  flex-direction: column;
  gap: 17px;
}
.auth-form label {
  display: flex;
  flex-direction: column;
  gap: 7px;
}
.auth-form label > span {
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
.auth-form input[readonly] {
  color: var(--text-muted);
  background: var(--bg);
}
.auth-submit {
  min-height: 45px;
  justify-content: space-between;
  margin-top: 4px;
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
    grid-template-columns: minmax(0, 1.35fr) minmax(320px, 0.8fr);
  }
}
@media (max-width: 899px) {
  .auth-page {
    min-height: auto;
    align-items: start;
    gap: 24px;
    padding: 12px 0 max(32px, env(safe-area-inset-bottom));
  }
  .device-plate {
    margin-top: 22px;
  }
  .session-steps {
    display: none;
  }
  .auth-panel {
    padding: 20px;
  }
  .auth-form input {
    scroll-margin-top: calc(72px + env(safe-area-inset-top));
    scroll-margin-bottom: 35vh;
  }
}
</style>
