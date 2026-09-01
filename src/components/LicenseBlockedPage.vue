<script setup lang="ts">
import { onMounted, onUnmounted, reactive } from "vue"
import { tauriApi } from "../services/tauri"

withDefaults(defineProps<{ deviceId?: string; message?: string | null }>(), {
  deviceId: "",
  message: "",
})

const exit = reactive({ remaining: 30, active: false })
let countdownTimer: ReturnType<typeof setInterval> | undefined

const exitApp = async () => {
  if (exit.active) return
  exit.active = true
  try {
    await tauriApi.exitApp()
  } catch {
    exit.active = false
  }
}

onMounted(() => {
  countdownTimer = setInterval(() => {
    exit.remaining -= 1
    if (exit.remaining <= 0) {
      clearInterval(countdownTimer)
      void exitApp()
    }
  }, 1000)
})
onUnmounted(() => clearInterval(countdownTimer))
</script>

<template>
  <main class="license-blocked">
    <section class="license-card" aria-labelledby="license-title">
      <div class="license-mark" aria-hidden="true">!</div>
      <span class="license-kicker">DEVICE AUTHORIZATION</span>
      <h1 id="license-title">设备未通过授权校验</h1>
      <p class="license-copy">当前设备未授权</p>

      <p v-if="message" class="license-error">{{ message }}</p>

      <div class="device-id-box">
        <span>设备 ID</span>
        <code>{{ deviceId || "无法读取设备 ID" }}</code>
      </div>

      <button type="button" class="exit-button" :disabled="exit.active" @click="exitApp">
        {{ exit.active ? "正在退出…" : "退出应用" }}
      </button>
      <small class="exit-hint">{{ exit.remaining }} 秒后自动退出应用</small>
    </section>
  </main>
</template>

<style scoped>
.license-blocked {
  min-height: 100vh;
  min-height: 100dvh;
  display: grid;
  place-items: center;
  padding: 24px;
  color: #dbe8e9;
  background: #0b1826;
}

.license-card {
  width: min(100%, 540px);
  padding: clamp(25px, 5vw, 42px);
  border: 1px solid rgba(139, 224, 219, 0.18);
  border-radius: 14px;
  background: #102b38;
  box-shadow: 0 22px 70px rgba(0, 0, 0, 0.3);
}

.license-mark {
  display: grid;
  place-items: center;
  width: 40px;
  height: 40px;
  margin-bottom: 22px;
  border: 1px solid #f09a4b;
  border-radius: 50%;
  color: #f09a4b;
  font:
    800 1.1rem/1 "Cascadia Code",
    monospace;
}

.license-kicker {
  color: #63c9c4;
  font:
    700 0.62rem/1 "Cascadia Code",
    monospace;
  letter-spacing: 0.14em;
}

.license-card h1 {
  margin: 10px 0 12px;
  color: #efffff;
  font-size: clamp(1.45rem, 4vw, 2rem);
  letter-spacing: -0.045em;
}

.license-copy {
  margin: 0;
  color: #9bb2b8;
  font-size: 0.78rem;
  line-height: 1.75;
}

.license-error {
  margin: 14px 0 0;
  color: #f09a4b;
  font-size: 0.74rem;
  line-height: 1.6;
  overflow-wrap: anywhere;
}

.device-id-box {
  display: grid;
  gap: 8px;
  margin-top: 24px;
  padding: 14px;
  border: 1px solid rgba(99, 201, 196, 0.22);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.18);
}

.device-id-box span {
  color: #71949c;
  font-size: 0.65rem;
}

.device-id-box code {
  color: #8de1dc;
  font:
    700 0.76rem/1.55 "Cascadia Code",
    monospace;
  overflow-wrap: anywhere;
  user-select: text;
}

.exit-button {
  width: 100%;
  margin-top: 24px;
  padding: 11px 16px;
  border: 1px solid #44c8c1;
  border-radius: 7px;
  color: #08212c;
  background: #63d4ce;
  font: 700 0.78rem/1.2 inherit;
  cursor: pointer;
}

.exit-button:hover:not(:disabled) {
  background: #8de1dc;
}

.exit-button:disabled {
  opacity: 0.65;
  cursor: wait;
}

.exit-hint {
  display: block;
  margin-top: 10px;
  color: #71949c;
  font-size: 0.62rem;
  text-align: center;
}
</style>
