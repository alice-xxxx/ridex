<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, reactive, ref, watch } from "vue"
import { storeToRefs } from "pinia"
import { formatContentValue, runtimeItemKey } from "../registers/codec"
import { useComponentStatusStore } from "../stores/componentStatus"
import { useSessionStore } from "../stores/session"
import { useTerminalStore } from "../stores/terminal"
import ComponentPicker from "./ComponentPicker.vue"
import DeviceBackButton from "./DeviceBackButton.vue"
import RegCard from "./RegCard.vue"
import type { StatusCategory } from "../types"

const status = useComponentStatusStore()
const session = useSessionStore()
const terminal = useTerminalStore()
const { data, inCooldown } = storeToRefs(status)
const { reading, writing } = status
const { ota } = terminal
const ui = reactive({ categoryId: "", query: "" })
const filtersOpen = ref(false)
const toolbar = ref<HTMLElement | null>(null)
const searchInput = ref<HTMLInputElement | null>(null)
const categories = computed(() => data.value.categories)
const currentCategory = computed(() => categories.value.find((entry) => entry.id === ui.categoryId)?.name ?? "状态")

function toggleFilters(focusSearch = false) {
  filtersOpen.value = focusSearch || !filtersOpen.value
  if (focusSearch) void nextTick(() => searchInput.value?.focus())
}

function closeFilters(event: PointerEvent) {
  if (!toolbar.value?.contains(event.target as Node)) filtersOpen.value = false
}

onMounted(() => document.addEventListener("pointerdown", closeFilters))
onUnmounted(() => document.removeEventListener("pointerdown", closeFilters))
const categoryItems = (category: StatusCategory) => [
  ...category.registers.map((item) => ({ item, categoryName: category.name })),
  ...category.coils.map((item) => ({ item, categoryName: category.name })),
]

watch(
  categories,
  (next) => {
    if (!next.some((category) => category.id === ui.categoryId)) {
      ui.categoryId = next[0]?.id ?? ""
    }
  },
  { immediate: true },
)

const availableItems = computed(() => {
  if (ui.query.trim()) return categories.value.flatMap(categoryItems)
  const category = categories.value.find((entry) => entry.id === ui.categoryId)
  return category ? categoryItems(category) : []
})
const visibleItems = computed(() => {
  const query = ui.query.trim().toLocaleLowerCase()
  if (!query) return availableItems.value
  return availableItems.value.filter(({ item, categoryName }) => {
    const content = item.content.flatMap((entry) => [
      entry.label,
      entry.on,
      entry.off,
      entry.unit,
      ...((entry.options ?? []).map((option) => option.label)),
      formatContentValue(entry, item),
    ])
    return [categoryName, item.address, item.label, item.rawHex, ...content]
      .filter((value) => value !== null && value !== undefined)
      .join(" ")
      .toLocaleLowerCase()
      .includes(query)
  })
})
const progressText = computed(() =>
  reading.active ? `读取 ${reading.progress.done}/${reading.progress.total}` : "读取",
)
const progressPercent = computed(() =>
  reading.progress.total ? (reading.progress.done / reading.progress.total) * 100 : 0,
)
</script>

<template>
  <section class="page component-status-page" aria-label="部件状态">
    <div ref="toolbar" class="status-toolbar" @keydown.esc="filtersOpen = false">
      <DeviceBackButton />
      <button class="status-filter-trigger" type="button" aria-controls="status-filters"
        :aria-label="`选择部件和分类，当前 ${session.view.component}，${currentCategory}`"
        :aria-expanded="filtersOpen" @click="toggleFilters()">
        <span class="status-current-component">{{ session.view.component }}</span>
        <span class="status-current-category">{{ currentCategory }}</span>
        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m3.5 6 4.5 4 4.5-4" /></svg>
      </button>
      <button class="status-search-trigger" type="button" aria-label="搜索状态" aria-controls="status-filters"
        :class="{ 'is-active': ui.query.trim() }"
        :aria-expanded="filtersOpen" @click="toggleFilters(true)">
        <svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="8.5" cy="8.5" r="5.5" /><path d="m13 13 4 4" /></svg>
      </button>

      <div id="status-filters" class="status-filters" :class="{ 'is-open': filtersOpen }">
        <ComponentPicker class="status-component-picker" />
        <nav class="status-categories" aria-label="状态分类">
          <button v-for="category in categories" :key="category.id" class="status-category"
            :class="{ active: ui.categoryId === category.id }" type="button" :aria-pressed="ui.categoryId === category.id"
            @click="ui.categoryId = category.id; ui.query = ''">
            {{ category.name }}
          </button>
        </nav>

        <label class="status-category-picker">
          <select v-model="ui.categoryId" aria-label="选择状态分类" @change="ui.query = ''">
            <option v-for="category in categories" :key="category.id" :value="category.id">
              {{ category.name }}
            </option>
          </select>
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <path d="m3.5 6 4.5 4 4.5-4" />
          </svg>
        </label>

        <label class="status-search">
          <svg viewBox="0 0 20 20" aria-hidden="true">
            <circle cx="8.5" cy="8.5" r="5.5" />
            <path d="m13 13 4 4" />
          </svg>
          <input ref="searchInput" v-model="ui.query" type="search" placeholder="搜索" aria-label="搜索" />
        </label>

        <button class="status-auto" type="button" role="switch" :aria-checked="reading.automatic" :disabled="ota.active"
          :aria-label="reading.automatic ? '关闭自动刷新' : '开启自动刷新'" @click="status.setAutoRefresh(!reading.automatic)">
          <span class="status-auto-track" aria-hidden="true"><i></i></span>
          <span>自动</span>
        </button>
      </div>

      <div class="status-search-actions">
        <button class="btn btn-outline status-refresh" type="button" :disabled="inCooldown || reading.active"
          :aria-busy="reading.active" :aria-label="reading.active ? progressText : '立即读取状态'" @click="status.refresh">
          <svg v-if="reading.active" class="status-progress" viewBox="0 0 20 20" aria-hidden="true">
            <circle class="status-progress-track" cx="10" cy="10" r="7.5" />
            <circle cx="10" cy="10" r="7.5" pathLength="100"
              :stroke-dasharray="`${progressPercent} 100`" transform="rotate(-90 10 10)" />
          </svg>
          <svg v-else viewBox="0 0 20 20" aria-hidden="true">
            <path d="M16.5 7A7 7 0 1 0 17 11" />
            <path d="M16.5 3v4h-4" />
          </svg>
          <span class="status-refresh-text">{{ progressText }}</span>
        </button>

        <button class="btn btn-outline status-terminal" type="button" aria-label="进入通信" title="通信"
          @click="session.goTo('terminal')">
          <svg viewBox="0 0 20 20" aria-hidden="true">
            <path d="M10 2.5v10m-3.5-3.5L10 12.5 13.5 9M3.5 14.5v3h13v-3" />
          </svg>
        </button>
      </div>
    </div>

    <div class="status-content">
      <div v-if="reading.error" class="msg msg-error" role="alert">{{ reading.error }}</div>
      <div v-if="writing.error" class="msg msg-error" role="alert">{{ writing.error }}</div>
      <div v-if="ota.active" class="msg msg-warn">OTA 更新期间，状态读取和修改已暂停。</div>

      <div v-if="!availableItems.length" class="component-empty">
        {{ ui.query.trim() ? "当前部件没有状态项" : "当前分类没有状态项" }}
      </div>
      <div v-else-if="!visibleItems.length" class="component-empty">当前部件没有匹配的内容</div>
      <div v-else class="component-list">
        <RegCard v-for="entry in visibleItems"
          :key="`${session.view.component}:${runtimeItemKey(entry.item)}`" :item="entry.item"
          :disabled="inCooldown" :writing="writing.key === runtimeItemKey(entry.item)" :write-error="writing.error"
          :write-register="status.writeRegister" :write-coil="status.writeCoil" />
      </div>
    </div>
  </section>
</template>

<style scoped>
.component-status-page {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}

.status-toolbar {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 0.5rem;
  min-width: 0;
  margin-bottom: 0.375rem;
  padding: 0.5rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  box-shadow: var(--shadow-sm);
}

.status-content {
  flex: 1;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
}

.status-category-picker {
  display: none;
}

.status-filters {
  display: contents;
}

.status-filter-trigger,
.status-search-trigger {
  display: none;
}

.status-component-picker {
  flex: 0 0 12.375rem;
}

.status-categories {
  display: flex;
  align-items: stretch;
  gap: 0.125rem;
  min-width: 0;
  overflow-x: auto;
  scrollbar-width: none;
}

.status-categories::-webkit-scrollbar {
  display: none;
}

.status-category {
  min-height: 2.25rem;
  padding: 0.375rem 0.5625rem;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  background: transparent;
  cursor: pointer;
  font-size: 0.74rem;
  white-space: nowrap;
}

.status-category:hover {
  background: var(--surface-hover);
}

.status-category.active {
  color: var(--primary);
  background: var(--primary-light);
  font-weight: 650;
}

.status-search-actions {
  display: contents;
}

.status-search {
  position: relative;
  display: flex;
  align-items: center;
  flex: 1 1 13.125rem;
  min-width: 9.375rem;
  height: 2.25rem;
  padding: 0 0.625rem;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-md);
  color: var(--text-muted);
  background: var(--surface);
}

.status-search:focus-within {
  border-color: var(--primary);
  box-shadow: 0 0 0 3px var(--primary-light);
}

.status-search svg,
.status-refresh svg,
.status-terminal svg,
.status-category-picker svg {
  fill: none;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.7;
}

.status-search svg {
  width: 16px;
  height: 16px;
}

.status-search input {
  min-width: 0;
  width: 100%;
  height: 100%;
  padding: 0 0.4375rem;
  border: 0;
  outline: 0;
  color: var(--text-primary);
  background: transparent;
  font-size: 0.76rem;
}

.status-refresh {
  min-width: 4.875rem;
  min-height: 2.25rem;
  padding: 0.375rem 0.625rem;
}

.status-refresh[aria-busy="true"] {
  opacity: 1;
  cursor: wait;
}

.status-refresh svg {
  width: 1.0625rem;
  height: 1.0625rem;
}

.status-refresh .status-progress {
  width: 1.25rem;
  height: 1.25rem;
}

.status-progress circle {
  stroke-width: 2.2;
  transition: stroke-dasharray 0.2s ease;
}

.status-progress-track {
  stroke: var(--border-strong);
}

.status-terminal {
  min-width: 2.25rem;
  min-height: 2.25rem;
  padding: 0;
}

.status-terminal svg {
  width: 1.125rem;
  height: 1.125rem;
}

.status-auto {
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
  min-height: 2.25rem;
  padding: 0.25rem;
  border: 0;
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  background: transparent;
  cursor: pointer;
  font-size: 0.72rem;
  font-weight: 600;
  white-space: nowrap;
}

.status-auto:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.status-auto-track {
  position: relative;
  width: 1.5rem;
  height: 14px;
  border: 1px solid var(--border-strong);
  border-radius: 62.4375rem;
  background: var(--gray);
}

.status-auto-track i {
  position: absolute;
  top: 0.125rem;
  left: 0.125rem;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--surface);
  box-shadow: var(--shadow-sm);
  transition: transform var(--transition-fast);
}

.status-auto[aria-checked="true"] .status-auto-track {
  border-color: var(--primary);
  background: var(--primary);
}

.status-auto[aria-checked="true"] .status-auto-track i {
  transform: translateX(10px);
}

.component-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  min-width: 0;
  padding-bottom: calc(0.5rem + env(safe-area-inset-bottom));
}

.component-empty {
  padding: 2.5rem 0.75rem;
  color: var(--text-muted);
  text-align: center;
}

@media (max-width: 1100px) {
  .status-toolbar {
    flex-wrap: wrap;
  }

  .status-categories {
    flex: 1 0 100%;
  }
}

@media (max-width: 767px) {
  .status-toolbar {
    position: relative;
    z-index: 2;
    display: grid;
    grid-template-columns: 2.25rem minmax(0, 1fr) repeat(3, 2.25rem);
    gap: 0.375rem;
    margin-bottom: 0.5rem;
    padding: 0 0 0.5rem;
    border: 0;
    border-bottom: 1px solid var(--border);
    border-radius: 0;
    background: transparent;
    box-shadow: none;
  }

  .status-toolbar > :deep(button:first-child) {
    width: 2.25rem;
    height: 2.25rem;
  }

  .status-toolbar > :deep(button:first-child svg) {
    width: 18px;
    height: 18px;
  }

  .status-filter-trigger {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    grid-column: 2;
    min-width: 0;
    padding: 0 0.125rem;
    border: 0;
    color: var(--text-primary);
    background: transparent;
    cursor: pointer;
    font-size: 0.8125rem;
    font-weight: 700;
    white-space: nowrap;
  }

  .status-current-component {
    flex: 0 0 auto;
    color: var(--primary);
  }

  .status-current-category {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .status-filter-trigger svg {
    flex: 0 0 12px;
    width: 12px;
    fill: none;
    stroke: var(--text-muted);
    stroke-linecap: round;
    stroke-linejoin: round;
    stroke-width: 1.7;
  }

  .status-search-trigger,
  .status-refresh,
  .status-terminal {
    display: grid;
    place-items: center;
    width: 2.25rem;
    height: 2.25rem;
    min-width: 0;
    min-height: 0;
    padding: 0;
    border: 1px solid var(--border-strong);
    border-radius: 50%;
    color: var(--primary);
    background: var(--surface);
    cursor: pointer;
  }

  .status-search-trigger {
    grid-column: 3;
  }

  .status-search-trigger.is-active {
    border-color: var(--primary);
    background: var(--primary-light);
  }

  .status-search-trigger svg {
    width: 17px;
    height: 17px;
    fill: none;
    stroke: currentColor;
    stroke-linecap: round;
    stroke-width: 1.7;
  }

  .status-search-actions {
    display: contents;
  }

  .status-refresh {
    grid-column: 4;
  }

  .status-terminal {
    grid-column: 5;
  }

  .status-filters {
    position: absolute;
    top: calc(100% + 0.25rem);
    right: 0;
    left: 0;
    display: none;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.5rem;
    padding: 0.75rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow: var(--shadow-md);
  }

  .status-filters.is-open {
    display: grid;
  }

  .status-component-picker {
    width: 100% !important;
    min-width: 0 !important;
  }

  .status-component-picker :deep(select),
  .status-category-picker select,
  .status-search,
  .status-auto {
    width: 100%;
    height: 2.25rem;
    min-height: 2.25rem;
    background: var(--bg);
  }

  .status-component-picker :deep(select),
  .status-category-picker select {
    padding: 0 1.75rem 0 0.625rem;
    font-size: 0.8125rem;
  }

  .status-category-picker {
    position: relative;
    display: block;
    min-width: 0;
  }

  .status-category-picker select {
    appearance: none;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-md);
    color: var(--text-primary);
    font-weight: 600;
  }

  .status-category-picker svg {
    position: absolute;
    top: 50%;
    right: 0.625rem;
    width: 15px;
    pointer-events: none;
    transform: translateY(-50%);
  }

  .status-categories {
    display: none;
  }

  .status-search {
    min-width: 0;
    padding: 0 0.625rem;
  }

  .status-auto {
    justify-content: center;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-md);
  }

  .status-refresh-text {
    display: none;
  }
}

@media (max-width: 360px) {
  .status-current-component {
    display: none;
  }
}

@media (min-width: 1101px) {
  .component-list {
    display: block;
    column-count: 2;
    column-gap: 0.625rem;
  }

  .component-list :deep(.rc-card) {
    break-inside: avoid;
    margin-bottom: 0.625rem;
  }
}

</style>
