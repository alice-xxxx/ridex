<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, useId } from "vue"
import { devices } from "../registers/components"

defineProps<{ label: string }>()
const value = defineModel<number>({ required: true })
const open = ref(false)
const root = ref<HTMLElement>()
const menuId = useId()

function toggle() {
  open.value = !open.value
}

function select(address: number) {
  value.value = address
  open.value = false
}

function closeWhenOutside(event: PointerEvent) {
  if (!root.value?.contains(event.target as Node)) open.value = false
}

onMounted(() => document.addEventListener("pointerdown", closeWhenOutside))
onBeforeUnmount(() => document.removeEventListener("pointerdown", closeWhenOutside))
</script>

<template>
  <div ref="root" class="device-address-picker" @keydown.esc="open = false">
    <input
      v-model.number="value"
      type="number"
      min="0"
      max="255"
      class="input input-sm device-address-input"
      :aria-label="label"
      :aria-expanded="open"
      :aria-controls="menuId"
      @focus="open = false"
    />
    <button
      type="button"
      class="device-address-toggle"
      :aria-label="`选择${label}`"
      :aria-expanded="open"
      :aria-controls="menuId"
      aria-haspopup="listbox"
      @click="toggle"
    >
      ▾
    </button>
    <div v-if="open" :id="menuId" class="device-address-menu" role="listbox" :aria-label="label">
      <button
        v-for="device in devices"
        :key="device.address"
        type="button"
        class="device-address-option"
        :class="{ selected: value === device.address }"
        role="option"
        :aria-selected="value === device.address"
        @click="select(device.address)"
      >
        <strong>{{ device.address }}</strong><span>{{ device.name }}</span>
      </button>
      <small class="device-address-hint">可直接输入自定义地址</small>
    </div>
  </div>
</template>

<style scoped>
.device-address-picker {
  position: relative;
  min-width: 0;
}

.device-address-input {
  padding-right: 34px !important;
  appearance: textfield;
  -moz-appearance: textfield;
}

.device-address-input::-webkit-inner-spin-button,
.device-address-input::-webkit-outer-spin-button {
  margin: 0;
  appearance: none;
}

.device-address-toggle {
  position: absolute;
  top: 3px;
  right: 3px;
  display: grid;
  place-items: center;
  width: 28px;
  height: calc(100% - 6px);
  padding: 0;
  border: 0;
  border-radius: 5px;
  color: var(--text-secondary);
  background: transparent;
  cursor: pointer;
  font: 800 0.85rem/1 "Cascadia Code", monospace;
}

.device-address-toggle:hover,
.device-address-toggle[aria-expanded="true"] {
  background: var(--primary-light);
}

.device-address-menu {
  position: absolute;
  z-index: 110;
  top: calc(100% + 6px);
  left: 0;
  width: 100%;
  min-width: 156px;
  max-height: min(360px, calc(100dvh - 170px));
  overflow-y: auto;
  padding: 4px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface);
  box-shadow: var(--shadow-md), 0 8px 24px rgba(10, 32, 44, 0.12);
}

.device-address-option {
  display: grid;
  grid-template-columns: 28px 1fr;
  align-items: center;
  width: 100%;
  min-height: 30px;
  padding: 5px 7px;
  border: 0;
  border-radius: 5px;
  color: var(--text-secondary);
  background: transparent;
  cursor: pointer;
  text-align: left;
}

.device-address-option:hover,
.device-address-option.selected {
  color: var(--primary);
  background: var(--primary-light);
}

.device-address-option strong {
  font: 750 0.68rem/1 "Cascadia Code", monospace;
}

.device-address-option span {
  overflow: hidden;
  font-size: 0.68rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.device-address-hint {
  display: block;
  margin: 4px 7px 2px;
  padding-top: 5px;
  border-top: 1px solid var(--border);
  color: var(--text-muted);
  font-size: 0.57rem;
}
</style>
