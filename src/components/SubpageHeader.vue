<script setup lang="ts">
import RoundBackButton from "./RoundBackButton.vue"

defineProps<{ title: string; backLabel: string; disabled?: boolean }>()
defineEmits<{ back: [] }>()
</script>

<template>
  <header class="subpage-header">
    <RoundBackButton :label="backLabel" :disabled="disabled" @back="$emit('back')" />
    <h1>{{ title }}</h1>
  </header>
</template>

<style scoped>
.subpage-header {
  display: grid;
  grid-template-columns: 2.5rem minmax(0, 1fr) 2.5rem;
  align-items: center;
  min-height: 2.5rem;
  margin-bottom: 1rem;
}

h1 {
  grid-column: 2;
  grid-row: 1;
  margin: 0;
  overflow: hidden;
  color: var(--text-primary);
  font-size: 1.125rem;
  font-weight: 700;
  line-height: 1.3;
  text-align: center;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
