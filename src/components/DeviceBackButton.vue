<script setup lang="ts">
import { useSessionStore } from "../stores/session"

const session = useSessionStore()
</script>

<template>
  <button type="button" class="btn btn-outline device-back-button" aria-label="返回设备搜索"
    :disabled="session.link.disconnecting || session.link.connecting" @click="session.returnToSearch">
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.8"
      stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="m12 5-7 7 7 7M5 12h14" />
    </svg>
  </button>
</template>

<style scoped>
.device-back-button {
  display: grid;
  place-items: center;
  width: 2.5rem;
  height: 2.5rem;
  flex: 0 0 auto;
  padding: 0;
}
</style>
