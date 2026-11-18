<script setup lang="ts">
import { useSessionStore } from "../stores/session"
import RoundBackButton from "./RoundBackButton.vue"

const session = useSessionStore()
</script>

<template>
  <RoundBackButton label="返回设备搜索" :disabled="session.link.disconnecting || session.link.connecting"
    @back="session.returnToSearch" />
</template>
