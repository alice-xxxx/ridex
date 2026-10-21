<script setup lang="ts">
withDefaults(defineProps<{ disabled?: boolean; loading?: boolean }>(), {
  disabled: false,
  loading: false,
})

defineEmits<{ click: [] }>()
</script>

<template>
  <button type="button" :disabled="disabled" title="断开连接" aria-label="断开设备连接" @click="$emit('click')">
    <span v-if="loading" class="spinner-sm" aria-hidden="true"></span>
    <svg v-else viewBox="0 0 20 20" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.8"
      aria-hidden="true">
      <path d="M7 3v3M13 3v3M3 8h14M5 5h10a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2V7a2 2 0 012-2z" />
    </svg>
  </button>
</template>

<style scoped>
.spinner-sm {
  width: 14px;
  height: 14px;
  border: 2px solid transparent;
  border-top-color: currentColor;
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
}
</style>
