<script setup lang="ts">
import { computed, reactive, watch } from "vue"
import { storeToRefs } from "pinia"
import { formatContentValue, runtimeItemKey } from "../registers/codec"
import { useComponentStatusStore } from "../stores/componentStatus"
import { useSessionStore } from "../stores/session"
import { useTerminalStore } from "../stores/terminal"
import ComponentPicker from "./ComponentPicker.vue"
import RegCard from "./RegCard.vue"
import type { StatusCategory } from "../types"

const status = useComponentStatusStore()
const session = useSessionStore()
const terminal = useTerminalStore()
const { data, inCooldown } = storeToRefs(status)
const { reading, writing } = status
const { ota } = terminal
const ui = reactive({ categoryId: "", query: "" })

const categories = computed(() => data.value.categories)
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

const allItems = computed(() => categories.value.flatMap(categoryItems))
const availableItems = computed(() => {
  if (ui.query.trim()) return allItems.value
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
</script>

<template>
  <section class="page component-status-page" aria-label="部件状态">
    <div class="status-toolbar">
      <ComponentPicker class="status-component-picker" />
      <nav class="status-categories" aria-label="状态分类">
        <button
          v-for="category in categories"
          :key="category.id"
          class="status-category"
          :class="{ active: ui.categoryId === category.id }"
          type="button"
          :aria-pressed="ui.categoryId === category.id"
          @click="ui.categoryId = category.id; ui.query = ''"
        >
          {{ category.name }}
        </button>
      </nav>

      <label class="status-category-picker">
        <select v-model="ui.categoryId" aria-label="选择状态分类" @change="ui.query = ''">
          <option v-for="category in categories" :key="category.id" :value="category.id">
            {{ category.name }}
          </option>
        </select>
        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m3.5 6 4.5 4 4.5-4" /></svg>
      </label>

      <div class="status-search-actions">
        <label class="status-search">
          <svg viewBox="0 0 20 20" aria-hidden="true">
            <circle cx="8.5" cy="8.5" r="5.5" />
            <path d="m13 13 4 4" />
          </svg>
          <input v-model="ui.query" type="search" placeholder="搜索" aria-label="搜索" />
        </label>

        <button
          class="btn btn-outline status-refresh"
          type="button"
          :disabled="inCooldown || reading.active"
          :aria-busy="reading.active"
          :aria-label="reading.active ? progressText : '立即读取状态'"
          @click="status.refresh"
        >
          <span v-if="reading.active" class="spinner" aria-hidden="true"></span>
          <svg v-else viewBox="0 0 20 20" aria-hidden="true">
            <path d="M16.5 7A7 7 0 1 0 17 11" />
            <path d="M16.5 3v4h-4" />
          </svg>
          <span class="status-refresh-text">{{ progressText }}</span>
        </button>

        <button
          class="status-auto"
          type="button"
          role="switch"
          :aria-checked="reading.automatic"
          :disabled="ota.active"
          :aria-label="reading.automatic ? '关闭自动刷新' : '开启自动刷新'"
          @click="status.setAutoRefresh(!reading.automatic)"
        >
          <span class="status-auto-track" aria-hidden="true"><i></i></span>
          <span>自动</span>
        </button>
      </div>
    </div>

    <div
      v-if="reading.active"
      class="component-progress"
      role="progressbar"
      :aria-valuenow="reading.progress.done"
      :aria-valuemax="reading.progress.total"
    >
      <i
        :style="{
          width: `${reading.progress.total ? (reading.progress.done / reading.progress.total) * 100 : 0}%`,
        }"
      ></i>
    </div>
    <div v-if="reading.error" class="msg msg-error" role="alert">{{ reading.error }}</div>
    <div v-if="writing.error" class="msg msg-error" role="alert">{{ writing.error }}</div>
    <div v-if="ota.active" class="msg msg-warn">OTA 更新期间，状态读取和修改已暂停。</div>

    <div v-if="!availableItems.length" class="component-empty">
      {{ ui.query.trim() ? "当前部件没有状态项" : "当前分类没有状态项" }}
    </div>
    <div v-else-if="!visibleItems.length" class="component-empty">当前部件没有匹配的内容</div>
    <div v-else class="component-list">
      <RegCard
        v-for="entry in visibleItems"
        :key="`${session.view.component}:${runtimeItemKey(entry.item)}`"
        :item="entry.item"
        :disabled="inCooldown"
        :writing="writing.key === runtimeItemKey(entry.item)"
        :write-error="writing.error"
        :write-register="status.writeRegister"
        :write-coil="status.writeCoil"
      />
    </div>
  </section>
</template>

<style scoped>
.component-status-page {
  min-width: 0;
}

.status-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  margin-bottom: 10px;
  padding: 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  box-shadow: var(--shadow-sm);
}

.status-category-picker {
  display: none;
}

.status-component-picker {
  flex: 0 0 198px;
}

.status-categories {
  display: flex;
  align-items: stretch;
  gap: 2px;
  min-width: 0;
  overflow-x: auto;
  scrollbar-width: none;
}

.status-categories::-webkit-scrollbar {
  display: none;
}

.status-category {
  min-height: 36px;
  padding: 6px 9px;
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
  flex: 1 1 210px;
  min-width: 150px;
  height: 36px;
  padding: 0 10px;
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
  padding: 0 7px;
  border: 0;
  outline: 0;
  color: var(--text-primary);
  background: transparent;
  font-size: 0.76rem;
}

.status-refresh {
  min-width: 78px;
  min-height: 36px;
  padding: 6px 10px;
}

.status-refresh svg {
  width: 17px;
  height: 17px;
}

.status-auto {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  min-height: 36px;
  padding: 5px 8px;
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
  width: 28px;
  height: 16px;
  border: 1px solid var(--border-strong);
  border-radius: 999px;
  background: var(--gray);
}

.status-auto-track i {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 10px;
  height: 10px;
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
  transform: translateX(12px);
}

.component-progress {
  height: 2px;
  margin: 0 1px 8px;
  overflow: hidden;
  border-radius: 99px;
  background: var(--border);
}

.component-progress i {
  display: block;
  height: 100%;
  background: var(--primary);
  transition: width 0.2s ease;
}

.component-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
}

.component-empty {
  padding: 40px 12px;
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
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
    padding: 6px;
  }

  .status-categories {
    display: none;
  }

  .status-category-picker {
    display: block;
    grid-column: 2;
    grid-row: 1;
    min-width: 0;
  }

  .status-component-picker {
    grid-column: 1;
    grid-row: 1;
    width: 100% !important;
    min-width: 0 !important;
  }

  .status-category-picker {
    position: relative;
  }

  .status-category-picker select {
    width: 100%;
    min-height: 40px;
    padding: 7px 32px 7px 10px;
    appearance: none;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-md);
    color: var(--text-primary);
    background: var(--surface);
  }

  .status-category-picker svg {
    position: absolute;
    top: 50%;
    right: 10px;
    width: 15px;
    pointer-events: none;
    transform: translateY(-50%);
  }

  .status-search-actions {
    display: flex;
    grid-column: 1 / -1;
    grid-row: 2;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .status-search {
    flex: 1 1 auto;
    min-width: 0;
    height: 40px;
  }

  .status-refresh {
    flex: 0 0 44px;
    min-width: 44px;
    min-height: 40px;
    padding: 0;
  }

  .status-refresh-text {
    display: none;
  }

  .status-auto {
    min-height: 40px;
  }
}

@media (min-width: 1101px) {
  .component-list {
    display: block;
    column-count: 2;
    column-gap: 10px;
  }

  .component-list :deep(.rc-card) {
    break-inside: avoid;
    margin-bottom: 10px;
  }
}
</style>
