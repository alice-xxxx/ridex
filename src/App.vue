<script setup lang="ts">
import { onMounted, reactive } from "vue"
import { useSessionStore } from "./stores/session"
import { errorMessage } from "./services/protocol"
import { tauriApi } from "./services/tauri"
import { loadComponentCatalogs } from "./registers/components"
import AppHeader from "./components/AppHeader.vue"
import { checkForAppUpdate } from "./services/appUpdate"
import SearchPage from "./components/SearchPage.vue"
import AuthPage from "./components/AuthPage.vue"
import ComponentStatusPage from "./components/ComponentStatusPage.vue"
import CommPage from "./components/CommPage.vue"
import BinToOtaTool from "./components/BinToOtaTool.vue"
import LicenseBlockedPage from "./components/LicenseBlockedPage.vue"
import type { LicenseStatus } from "./types"

const session = useSessionStore()
const license = reactive({
  loading: true,
  status: { authorized: false, deviceId: "", message: null } as LicenseStatus,
})

const sleep = (ms: number) => new Promise<void>((resolve) => window.setTimeout(resolve, ms))

const readLicenseStatus = async (): Promise<LicenseStatus | null> => {
  try {
    return await Promise.race([
      tauriApi.licenseStatus(),
      new Promise<null>((resolve) => window.setTimeout(() => resolve(null), 250)),
    ])
  } catch {
    return null
  }
}

onMounted(async () => {
  try {
    const deadline = Date.now() + 6000
    while (Date.now() < deadline) {
      const status = await readLicenseStatus()
      if (status) {
        license.status = status
        break
      }
      await sleep(250)
    }
    if (!license.status.authorized && !license.status.message) {
      license.status.message = "应用启动超时，请重新启动"
    }
    if (license.status.authorized) await loadComponentCatalogs()
  } catch (error) {
    license.status = {
      authorized: false,
      deviceId: "",
      message: errorMessage(error, "无法读取设备授权状态"),
    }
  } finally {
    license.loading = false
    if (license.status.authorized) void checkForAppUpdate()
  }
})
</script>

<template>
  <main v-if="license.loading" class="startup-status" role="status" aria-live="polite">
    <span class="startup-spinner" aria-hidden="true"></span>
    <strong>启动中</strong>
  </main>
  <LicenseBlockedPage v-else-if="!license.status.authorized" :device-id="license.status.deviceId"
    :message="license.status.message" />
  <div v-else class="app" :class="{ 'app-terminal': session.page === 'terminal' }">
    <AppHeader />
    <main class="main">
      <div v-if="session.link.current && session.link.connectError" class="msg msg-error" role="alert">
        {{ session.link.connectError }}
      </div>
      <Transition name="page" mode="out-in">
        <SearchPage v-if="session.page === 'search'" key="search" />
        <AuthPage v-else-if="session.page === 'auth'" key="auth" />
        <ComponentStatusPage v-else-if="session.page === 'status'" key="status" />
        <BinToOtaTool v-else-if="session.page === 'ota-tool'" key="ota-tool" :device-id="license.status.deviceId" />
        <CommPage v-else key="terminal" />
      </Transition>
    </main>

    <nav v-if="session.authenticated" class="bottom-nav" aria-label="页面导航">
      <button class="bottom-nav-item" :class="{ active: session.page === 'status' }"
        :aria-current="session.page === 'status' ? 'page' : undefined" @click="session.goTo('status')">
        <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.8">
          <rect x="3" y="3" width="7" height="7" rx="1.5" />
          <rect x="14" y="3" width="7" height="7" rx="1.5" />
          <rect x="3" y="14" width="7" height="7" rx="1.5" />
          <rect x="14" y="14" width="7" height="7" rx="1.5" />
        </svg>
        <span>状态</span>
      </button>
      <button class="bottom-nav-item" :class="{ active: session.page === 'terminal' }"
        :aria-current="session.page === 'terminal' ? 'page' : undefined" @click="session.goTo('terminal')">
        <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.8">
          <path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z" />
        </svg>
        <span>通信</span>
      </button>
      <button class="bottom-nav-item" :class="{ active: session.page === 'ota-tool' }"
        :aria-current="session.page === 'ota-tool' ? 'page' : undefined" @click="session.goTo('ota-tool')">
        <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.8">
          <rect x="6" y="6" width="12" height="12" rx="2" />
          <path d="M9 2v4m6-4v4M9 18v4m6-4v4M2 9h4m-4 6h4m12-6h4m-4 6h4" />
        </svg>
        <span>固件工具</span>
      </button>
    </nav>
  </div>
</template>

<style scoped>
.startup-status {
  min-height: 100vh;
  min-height: 100dvh;
  display: grid;
  place-content: center;
  justify-items: center;
  gap: 0.5rem;
  padding: env(safe-area-inset-top) max(1.5rem, env(safe-area-inset-right)) env(safe-area-inset-bottom) max(1.5rem, env(safe-area-inset-left));
  color: var(--text-secondary);
}

.startup-status strong {
  color: var(--text-primary);
  font-size: 0.95rem;
}

.startup-spinner {
  width: 1.625rem;
  height: 1.625rem;
  margin-bottom: 0.25rem;
  border: 2px solid var(--border-strong);
  border-top-color: var(--primary);
  border-radius: 50%;
  animation: startup-spin 0.8s linear infinite;
}

@keyframes startup-spin {
  to {
    transform: rotate(360deg);
  }
}

.app {
  min-height: 100vh;
  min-height: 100dvh;
  display: flex;
  flex-direction: column;
  padding-top: env(safe-area-inset-top);
}

.main {
  flex: 1;
  display: flex;
  flex-direction: column;
  width: 100%;
  padding: 1.5rem max(1.75rem, env(safe-area-inset-right)) calc(5rem + env(safe-area-inset-bottom)) max(1.75rem, env(safe-area-inset-left));
}

.bottom-nav {
  position: fixed;
  inset: auto 0 0;
  z-index: 100;
  display: flex;
  padding: 0.375rem 0 max(0.375rem, env(safe-area-inset-bottom));
  border-top: 1px solid var(--border);
  background: var(--surface-glass);
  backdrop-filter: var(--blur);
}

.bottom-nav-item {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.125rem;
  padding: 0.375rem 0;
  border: 0;
  color: var(--text-muted);
  background: none;
  font-size: 0.65rem;
  font-weight: 600;
  cursor: pointer;
}

.bottom-nav-item.active {
  color: var(--primary);
}

@media (max-width: 767px) {
  .app-terminal {
    height: 100vh;
    height: 100dvh;
  }

  .app-terminal .main {
    min-height: 0;
    overflow-y: auto;
  }
}

@media (min-width: 768px) {
  .app {
    height: 100dvh;
    overflow: hidden;
  }

  .main {
    max-width: 107.5rem;
    margin: 0 auto;
    min-height: 0;
    padding: 1.75rem max(clamp(1.25rem, 3vw, 2.625rem), env(safe-area-inset-right)) env(safe-area-inset-bottom) max(clamp(1.25rem, 3vw, 2.625rem), env(safe-area-inset-left));
    overflow-y: auto;
  }

  .bottom-nav {
    inset: auto auto calc(1rem + env(safe-area-inset-bottom)) 50%;
    width: max-content;
    padding: 0.3125rem;
    border: 1px solid var(--border);
    border-radius: 0.875rem;
    box-shadow: var(--shadow-md);
    transform: translateX(-50%);
  }

  .bottom-nav-item {
    flex: 0 0 auto;
    min-width: 5rem;
    padding: 0.5rem 0.75rem;
  }
}
</style>
