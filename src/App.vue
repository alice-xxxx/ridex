<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue"
import { useSessionStore } from "./stores/session"
import { errorMessage } from "./services/protocol"
import { tauriApi } from "./services/tauri"
import { loadComponentCatalogs } from "./registers/components"
import { checkForAppUpdate } from "./services/appUpdate"
import SearchPage from "./components/SearchPage.vue"
import AuthPage from "./components/AuthPage.vue"
import ComponentStatusPage from "./components/ComponentStatusPage.vue"
import CommPage from "./components/CommPage.vue"
import BinToOtaTool from "./components/BinToOtaTool.vue"
import SettingsPage from "./components/SettingsPage.vue"
import LicenseBlockedPage from "./components/LicenseBlockedPage.vue"
import type { LicenseStatus } from "./types"

const session = useSessionStore()
const toolMode = ref<"convert" | "4g">("convert")
const fourGVisited = ref(false)
const deviceNav = computed(() => session.page === "status" || session.page === "terminal")
const showBottomNav = computed(() => deviceNav.value || session.page === "search" || session.page === "settings")

function goHome() {
  session.goTo("search")
}

function openTool(mode: "convert" | "4g") {
  toolMode.value = mode
  if (mode === "4g") fourGVisited.value = true
  session.goTo("ota-tool")
}
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
  <div v-else class="app" :class="{ 'app-terminal': session.page === 'terminal', 'app-detail': !showBottomNav }">
    <main class="main">
      <div v-if="session.link.current && session.link.connectError" class="msg msg-error" role="alert">
        {{ session.link.connectError }}
      </div>
      <Transition name="page" mode="out-in">
        <KeepAlive include="BinToOtaTool,CommPage,SearchPage">
          <SearchPage v-if="session.page === 'search'" key="search" />
          <AuthPage v-else-if="session.page === 'auth'" key="auth" />
          <ComponentStatusPage v-else-if="session.page === 'status'" key="status" />
          <CommPage v-else-if="session.page === 'terminal'" key="terminal" />
          <BinToOtaTool v-else-if="session.page === 'ota-tool'" key="ota-tool"
            :initial-mode="toolMode" :four-g-visited="fourGVisited" :device-id="license.status.deviceId" />
          <SettingsPage v-else :key="session.page" :section="session.page"
            :device-id="license.status.deviceId" @open-tool="openTool" />
        </KeepAlive>
      </Transition>
    </main>

    <nav v-if="showBottomNav" class="bottom-nav" :aria-label="deviceNav ? '设备导航' : '主导航'">
      <template v-if="deviceNav">
        <button type="button" class="bottom-nav-item" :class="{ active: session.page === 'status' }"
          :aria-current="session.page === 'status' ? 'page' : undefined" @click="session.goTo('status')">
          <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.8">
            <rect x="3" y="3" width="7" height="7" rx="1.5" />
            <rect x="14" y="3" width="7" height="7" rx="1.5" />
            <rect x="3" y="14" width="7" height="7" rx="1.5" />
            <rect x="14" y="14" width="7" height="7" rx="1.5" />
          </svg>
          <span>状态</span>
        </button>
        <button type="button" class="bottom-nav-item" :class="{ active: session.page === 'terminal' }"
          :aria-current="session.page === 'terminal' ? 'page' : undefined" @click="session.goTo('terminal')">
          <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.8">
            <path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z" />
          </svg>
          <span>通信</span>
        </button>
      </template>
      <template v-else>
        <button type="button" class="bottom-nav-item" :class="{ active: session.page === 'search' }"
          :aria-current="session.page === 'search' ? 'page' : undefined" @click="goHome">
          <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.8">
            <path d="m3 10 9-7 9 7v10a1 1 0 0 1-1 1h-5v-7H9v7H4a1 1 0 0 1-1-1z" />
          </svg>
          <span>主页</span>
        </button>
        <button type="button" class="bottom-nav-item" :class="{ active: session.page === 'settings' }"
          :aria-current="session.page === 'settings' ? 'page' : undefined" @click="session.goTo('settings')">
          <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.8">
            <circle cx="12" cy="12" r="3" />
            <path d="m10 2-.5 2.3-2.1.9-2-1.3-2.5 2.5 1.3 2-.9 2.1L1 11v2l2.3.5.9 2.1-1.3 2 2.5 2.5 2-1.3 2.1.9L10 22h4l.5-2.3 2.1-.9 2 1.3 2.5-2.5-1.3-2 .9-2.1L23 13v-2l-2.3-.5-.9-2.1 1.3-2-2.5-2.5-2 1.3-2.1-.9L14 2z" />
          </svg>
          <span>设置</span>
        </button>
      </template>
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

.app-detail .main {
  padding-bottom: max(1.5rem, env(safe-area-inset-bottom));
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
  min-height: 3rem;
  padding: 0.375rem 0;
  border: 0;
  color: var(--text-muted);
  background: none;
  font-size: 0.75rem;
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
