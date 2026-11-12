<script setup lang="ts">
import { useSessionStore } from "../stores/session"
import DeviceDisconnectButton from "./DeviceDisconnectButton.vue"

const session = useSessionStore()
const { link } = session
</script>

<template>
  <div
    v-if="link.current && session.page !== 'status' && session.page !== 'auth' && session.page !== 'terminal' && session.page !== 'ota-tool'"
    class="mobile-device-dock">
    <strong>{{ link.current.name || "未命名设备" }}</strong>
    <DeviceDisconnectButton :disabled="link.disconnecting" :loading="link.disconnecting" @click="session.disconnect" />
  </div>
</template>

<style scoped>
.mobile-device-dock {
  display: none;
}

@media (max-width: 767px) {
  .mobile-device-dock {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.625rem;
    margin: 0.5rem 1.75rem 0;
    padding: 0.4375rem 0.625rem;
    border: 1px solid var(--border);
    border-radius: 0.5rem;
    color: var(--text-secondary);
    background: var(--surface);
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
