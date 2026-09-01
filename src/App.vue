<script setup lang="ts">
import { onMounted, reactive } from "vue"
import { useSessionStore } from "./stores/session"
import { errorMessage } from "./services/protocol"
import { tauriApi } from "./services/tauri"
import AppHeader from "./components/AppHeader.vue"
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

onMounted(async () => {
  try {
    const deadline = Date.now() + 2500
    while (Date.now() < deadline) {
      license.status = await tauriApi.licenseStatus()
      if (license.status.authorized || license.status.message) break
      await new Promise((resolve) => window.setTimeout(resolve, 150))
    }
    if (!license.status.authorized && !license.status.message) {
      license.status.message = "设备授权检查超时"
    }
  } catch (error) {
    license.status = {
      authorized: false,
      deviceId: "",
      message: errorMessage(error, "无法读取设备授权状态"),
    }
  } finally {
    license.loading = false
  }
})
</script>

<template>
  <LicenseBlockedPage
    v-if="!license.loading && !license.status.authorized"
    :device-id="license.status.deviceId"
    :message="license.status.message"
  />
  <div v-else-if="!license.loading" class="app">
    <AppHeader />
    <main class="main">
      <Transition name="page" mode="out-in">
        <SearchPage v-if="session.page === 'search'" key="search" />
        <AuthPage v-else-if="session.page === 'auth'" key="auth" />
        <ComponentStatusPage v-else-if="session.page === 'status'" key="status" />
        <BinToOtaTool v-else-if="session.page === 'ota-tool'" key="ota-tool" />
        <CommPage v-else key="terminal" />
      </Transition>
    </main>

    <nav v-if="session.authenticated" class="bottom-nav" aria-label="移动端导航">
      <button
        class="bottom-nav-item"
        :class="{ active: session.page === 'status' }"
        :aria-current="session.page === 'status' ? 'page' : undefined"
        @click="session.goTo('status')"
      >
        <svg
          viewBox="0 0 24 24"
          width="22"
          height="22"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
        >
          <rect x="3" y="3" width="7" height="7" rx="1.5" />
          <rect x="14" y="3" width="7" height="7" rx="1.5" />
          <rect x="3" y="14" width="7" height="7" rx="1.5" />
          <rect x="14" y="14" width="7" height="7" rx="1.5" />
        </svg>
        <span>状态</span>
      </button>
      <button
        class="bottom-nav-item"
        :class="{ active: session.page === 'terminal' }"
        :aria-current="session.page === 'terminal' ? 'page' : undefined"
        @click="session.goTo('terminal')"
      >
        <svg
          viewBox="0 0 24 24"
          width="22"
          height="22"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
        >
          <path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z" />
        </svg>
        <span>通信</span>
      </button>
    </nav>
  </div>
</template>

<style scoped>
.app {
  min-height: 100vh;
  min-height: 100dvh;
  display: flex;
  flex-direction: column;
}
.main {
  flex: 1;
  width: 100%;
  padding: 24px max(28px, env(safe-area-inset-right)) calc(80px + env(safe-area-inset-bottom))
    max(28px, env(safe-area-inset-left));
}
.bottom-nav {
  position: fixed;
  inset: auto 0 0;
  z-index: 100;
  display: flex;
  padding: 6px 0 max(6px, env(safe-area-inset-bottom));
  border-top: 1px solid var(--border);
  background: var(--surface-glass);
  backdrop-filter: var(--blur);
}
.bottom-nav-item {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  padding: 6px 0;
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
@media (min-width: 768px) {
  .app {
    padding-left: 236px;
  }
  .main {
    max-width: 1720px;
    margin: 0 auto;
    padding: 28px max(clamp(20px, 3vw, 42px), env(safe-area-inset-right)) 28px
      max(clamp(20px, 3vw, 42px), env(safe-area-inset-left));
  }
  .bottom-nav {
    display: none;
  }
}
</style>
