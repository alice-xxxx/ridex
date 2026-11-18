<script setup lang="ts">
defineProps<{ label: string; disabled?: boolean }>()
defineEmits<{ back: [] }>()
</script>

<template>
  <button type="button" :aria-label="label" :disabled="disabled" @click="$emit('back')">
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor"
      stroke-width="2.3" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="m15 4-8 8 8 8" />
    </svg>
  </button>
</template>

<style scoped>
button {
  display: grid;
  place-items: center;
  width: 2.5rem;
  height: 2.5rem;
  flex: 0 0 auto;
  padding: 0;
  border: 1px solid var(--border-strong);
  border-radius: 50%;
  color: var(--text-primary);
  background: var(--surface-hover);
  box-shadow: var(--shadow-sm);
  cursor: pointer;
}

button:disabled {
  opacity: 0.5;
  cursor: wait;
}
</style>
