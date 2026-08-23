<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from "vue"
import { storeToRefs } from "pinia"
import { componentDefinitions } from "../registers/components.js"
import { useSessionStore } from "../stores/session.js"

const session = useSessionStore()
const { component } = storeToRefs(session)
const options = Object.values(componentDefinitions)
const root = ref(null)
const open = ref(false)
const selectedOption = computed(() => options.find((option) => option.id === component.value) ?? options[0])

function componentCode(option) {
  return {
    vehicle: "VCU",
    bms: "BMS",
    ble: "BLE",
    controller: "MCU",
    abs: "ABS",
  }[option?.id] ?? option?.id?.toUpperCase() ?? ""
}

function toggleMenu() {
  open.value = !open.value
}

function selectOption(option) {
  session.selectComponent(option.id)
  open.value = false
}

function closeOnOutsidePointer(event) {
  if (!root.value?.contains(event.target)) open.value = false
}

function closeOnEscape(event) {
  if (event.key === "Escape") open.value = false
}

onMounted(() => {
  document.addEventListener("pointerdown", closeOnOutsidePointer)
  document.addEventListener("keydown", closeOnEscape)
})

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", closeOnOutsidePointer)
  document.removeEventListener("keydown", closeOnEscape)
})
</script>

<template>
  <div ref="root" class="component-picker">
    <button
      class="component-trigger"
      type="button"
      aria-haspopup="listbox"
      :aria-expanded="open"
      aria-label="切换状态部件"
      @click="toggleMenu"
    >
      <span class="component-trigger-copy">
        <strong>{{ componentCode(selectedOption) || "选择部件" }}</strong>
      </span>
      <svg class="component-trigger-arrow" :class="{ open }" viewBox="0 0 16 16" aria-hidden="true">
        <path d="m3.5 6 4.5 4 4.5-4" />
      </svg>
    </button>

    <div v-if="open" class="component-menu" role="listbox" aria-label="选择部件">
      <button
        v-for="option in options"
        :key="option.id"
        class="component-option"
        :class="{ active: option.id === component }"
        type="button"
        role="option"
        :aria-selected="option.id === component"
        @click="selectOption(option)"
      >
        <span class="component-option-code">{{ componentCode(option) }}</span>
        <svg v-if="option.id === component" viewBox="0 0 18 18" aria-hidden="true">
          <path d="m3.5 9.5 3.2 3.2 7.8-8" />
        </svg>
      </button>
    </div>
  </div>
</template>

<style scoped>
.component-picker {
  position: relative;
  width: 198px;
  min-width: 170px;
}

.component-trigger {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  width: 100%;
  min-height: 40px;
  padding: 7px 10px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-md);
  background: var(--surface);
  color: var(--text-primary);
  text-align: left;
  cursor: pointer;
  transition: border-color var(--transition-fast), box-shadow var(--transition-fast), background var(--transition-fast);
}

.component-trigger:hover,
.component-trigger[aria-expanded="true"] {
  border-color: var(--primary);
  box-shadow: 0 0 0 3px var(--primary-light);
}

.component-trigger-copy {
  display: flex;
  min-width: 0;
  flex: 1;
}

.component-trigger-copy strong {
  min-width: 0;
  overflow: hidden;
  color: var(--primary);
  font: 650 .76rem/1.2 'SF Mono', 'Cascadia Code', Consolas, monospace;
  letter-spacing: .03em;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.component-trigger-arrow {
  flex: 0 0 auto;
  width: 16px;
  height: 16px;
  fill: none;
  stroke: var(--text-secondary);
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.6;
  transition: transform var(--transition-fast), stroke var(--transition-fast);
}

.component-trigger-arrow.open {
  transform: rotate(180deg);
  stroke: var(--primary);
}

.component-menu {
  position: absolute;
  top: calc(100% + 6px);
  left: 0;
  right: 0;
  z-index: 40;
  max-height: min(280px, 50vh);
  overflow-y: auto;
  padding: 5px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-md);
  background: var(--surface);
  box-shadow: var(--shadow-lg);
}

.component-option {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  min-height: 34px;
  margin: 2px 0;
  padding: 5px 7px;
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-secondary);
  font-size: .74rem;
  text-align: left;
  cursor: pointer;
  transition: color var(--transition-fast), background var(--transition-fast);
}

.component-option:hover { background: var(--surface-hover); color: var(--text-primary); }
.component-option.active { background: var(--primary-light); color: var(--primary); }

.component-option-code {
  min-width: 0;
  overflow: hidden;
  color: var(--text-secondary);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.component-option.active .component-option-code {
  color: var(--primary);
}

.component-option > svg {
  width: 17px;
  height: 17px;
  flex: 0 0 auto;
  fill: none;
  stroke: var(--primary);
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.8;
}

@media (max-width: 620px) {
  .component-picker { width: min(198px, 100%); min-width: 0; }
  .component-menu { width: 100%; }
}

@media (prefers-reduced-motion: reduce) {
  .component-trigger,
  .component-trigger-arrow,
  .component-option { transition: none; }
}
</style>
