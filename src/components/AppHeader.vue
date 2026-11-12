<script setup lang="ts">
import { useSessionStore } from "../stores/session"
import DeviceDisconnectButton from "./DeviceDisconnectButton.vue"
import { version as appVersion } from "../../package.json"

const session = useSessionStore()
const { link } = session
</script>

<template>
  <header class="app-header">
    <span class="app-version-pill">Ride{{ appVersion }}</span>

    <div
      v-if="link.current && session.page !== 'status' && session.page !== 'auth' && session.page !== 'terminal' && session.page !== 'ota-tool'"
      class="mobile-device-dock">
      <strong>{{ link.current.name || "未命名设备" }}</strong>
      <DeviceDisconnectButton :disabled="link.disconnecting" :loading="link.disconnecting" @click="session.disconnect" />
    </div>
  </header>
</template>

<style scoped>
.app-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  padding: 0.875rem 1.75rem 0;
}

.app-version-pill {
  display: inline-flex;
  align-items: center;
  padding: 0.3rem 0.7rem;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: rgba(0, 143, 149, 0.08);
  color: var(--primary);
  font-size: 0.72rem;
  font-weight: 700;
  letter-spacing: 0.04em;
  white-space: nowrap;
}

.mobile-device-dock {
  display: none;
}

@media (max-width: 767px) {
  .app-header {
    padding-top: 0.75rem;
  }

  .mobile-device-dock {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.625rem;
    padding: 0.4375rem 0.625rem;
    border: 1px solid var(--border);
    border-radius: 0.5rem;
    color: var(--text-secondary);
    background: var(--surface);
    flex: 1;
    min-width: 0;
  }

  .mobile-device-dock strong {
    min-width: 0;
    overflow: hidden;
    font-size: 0.7rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .mobile-device-dock button {
    display: grid;
    place-items: center;
    width: 2rem;
    height: 1.875rem;
    flex: 0 0 auto;
    border: 0;
    border-radius: 0.375rem;
    color: var(--danger);
    background: var(--danger-light);
  }
}
</style>
