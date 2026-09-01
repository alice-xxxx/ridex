<script setup lang="ts">
import { devices } from "../registers/components"
import { useSessionStore } from "../stores/session"

const session = useSessionStore()
const components = devices.filter((device) => device.catalog)
</script>

<template>
  <label class="component-picker">
    <select v-model="session.view.component" aria-label="切换状态部件">
      <option v-for="device in components" :key="device.name" :value="device.name">
        {{ device.name }}
      </option>
    </select>
    <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m3.5 6 4.5 4 4.5-4" /></svg>
  </label>
</template>

<style scoped>
.component-picker {
  position: relative;
  display: block;
  width: 198px;
  min-width: 170px;
}

.component-picker select {
  width: 100%;
  min-height: 40px;
  padding: 7px 34px 7px 10px;
  appearance: none;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-md);
  outline: 0;
  color: var(--primary);
  background: var(--surface);
  cursor: pointer;
  font:
    650 0.76rem/1.2 "SF Mono",
    "Cascadia Code",
    Consolas,
    monospace;
  letter-spacing: 0.03em;
  transition:
    border-color var(--transition-fast),
    box-shadow var(--transition-fast);
}

.component-picker select:hover,
.component-picker select:focus-visible {
  border-color: var(--primary);
  box-shadow: 0 0 0 3px var(--primary-light);
}

.component-picker svg {
  position: absolute;
  top: 50%;
  right: 11px;
  width: 15px;
  pointer-events: none;
  transform: translateY(-50%);
  fill: none;
  stroke: var(--text-muted);
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.7;
}

@media (max-width: 720px) {
  .component-picker {
    width: 168px;
    min-width: 0;
  }
}
</style>
