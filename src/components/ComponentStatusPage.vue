<script setup>
import { computed, ref, watch } from "vue"
import { storeToRefs } from "pinia"
import { formatContentValue, runtimeItemKey } from "../registers/codec.js"
import { useComponentStatusStore } from "../stores/componentStatus.js"
import { useSessionStore } from "../stores/session.js"
import { useTerminalStore } from "../stores/terminal.js"
import RegCard from "./RegCard.vue"
import StatusToolbar from "./StatusToolbar.vue"

const status = useComponentStatusStore()
const session = useSessionStore()
const terminal = useTerminalStore()
const { data, refreshing, progress, error, writeError, writingKey, autoRefresh, inCooldown } = storeToRefs(status)
const { otaActive } = storeToRefs(terminal)

const activeCategoryId = ref("")
const query = ref("")
const categories = computed(() => data.value?.categories ?? [])

function categoryItems(category) {
  return [
    ...(category.registers ?? []).map((item) => ({ item, categoryName: category.name })),
    ...(category.coils ?? []).map((item) => ({ item, categoryName: category.name })),
  ]
}

watch(categories, (next) => {
  if (!next.some((category) => category.id === activeCategoryId.value)) activeCategoryId.value = next[0]?.id ?? ""
}, { immediate: true })

const activeCategory = computed(() => categories.value.find((category) => category.id === activeCategoryId.value) ?? null)
const activeItems = computed(() => activeCategory.value ? categoryItems(activeCategory.value) : [])
const componentItems = computed(() => categories.value.flatMap(categoryItems))
const normalizedQuery = computed(() => query.value.trim().toLocaleLowerCase())
const searchItems = computed(() => normalizedQuery.value ? componentItems.value : activeItems.value)
const visibleItems = computed(() => {
  if (!normalizedQuery.value) return activeItems.value
  return searchItems.value.filter(({ item, categoryName }) => {
    const content = (item.content ?? []).flatMap((entry) => [
      entry.label, entry.on, entry.off, entry.unit,
      ...(entry.options ?? []).map((option) => option.label),
      formatContentValue(entry, item),
    ])
    return [categoryName, item.address, item.label, item.rawHex, ...content]
      .filter((value) => value !== null && value !== undefined)
      .join(" ").toLocaleLowerCase().includes(normalizedQuery.value)
  })
})
const interactionDisabled = computed(() => Boolean(writingKey.value) || inCooldown.value || otaActive.value)
const progressText = computed(() => refreshing.value
  ? `读取 ${progress.value.done}/${progress.value.total}`
  : "读取")

function isWriting(item) {
  return writingKey.value === runtimeItemKey(item)
}

function selectCategory(id) {
  activeCategoryId.value = id
  query.value = ""
}
</script>

<template>
  <section class="page component-status-page" aria-label="部件状态">
    <StatusToolbar
      :categories="categories"
      :active-category-id="activeCategoryId"
      :query="query"
      :refreshing="refreshing"
      :progress-text="progressText"
      :refresh-disabled="interactionDisabled || refreshing"
      :auto-refresh="autoRefresh"
      :auto-disabled="otaActive"
      @select-category="selectCategory"
      @update:query="query = $event"
      @refresh="status.refresh"
      @toggle-auto="status.setAutoRefresh(!autoRefresh)"
    />

    <div v-if="refreshing" class="component-progress" role="progressbar" :aria-valuenow="progress.done" :aria-valuemax="progress.total">
      <i :style="{ width: `${progress.total ? (progress.done / progress.total) * 100 : 0}%` }"></i>
    </div>
    <div v-if="error" class="msg msg-error" role="alert">{{ error }}</div>
    <div v-if="writeError" class="msg msg-error" role="alert">{{ writeError }}</div>
    <div v-if="otaActive" class="msg msg-warn">OTA 更新期间，状态读取和修改已暂停。</div>

    <div v-if="!searchItems.length" class="component-empty">
      {{ normalizedQuery ? "当前部件没有状态项" : "当前分类没有状态项" }}
    </div>
    <div v-else-if="!visibleItems.length" class="component-empty">当前部件没有匹配的内容</div>
    <div v-else class="component-list">
      <RegCard
        v-for="entry in visibleItems"
        :key="`${session.component}:${runtimeItemKey(entry.item)}`"
        :item="entry.item"
        :disabled="interactionDisabled"
        :writing="isWriting(entry.item)"
        :write-error="writeError"
        :write-register="status.writeRegister"
        :write-coil="status.writeCoil"
      />
    </div>

  </section>
</template>

<style scoped>
.component-status-page { min-width: 0; }
.component-toolbar { display: grid; grid-template-columns: minmax(0, 1.1fr) minmax(230px, .8fr) auto; align-items: center; gap: 10px; margin-bottom: 12px; padding: 8px; border: 1px solid var(--border); border-radius: var(--radius-lg); background: var(--surface); box-shadow: var(--shadow-sm); }
.component-categories { display: flex; gap: 3px; min-width: 0; overflow-x: auto; padding: 0; scrollbar-width: none; }
.component-categories::-webkit-scrollbar { display: none; }
.component-categories button { flex: 0 0 auto; padding: 9px 10px; border: 0; border-radius: var(--radius-sm); color: var(--text-secondary); background: transparent; font-size: .72rem; white-space: nowrap; cursor: pointer; transition: color var(--transition-fast), background var(--transition-fast); }
.component-categories button.active { color: var(--primary); background: var(--primary-light); font-weight: 700; }
.component-categories small { margin-left: 4px; color: var(--text-muted); font: 600 .6rem/1 'Cascadia Code', monospace; }
.component-search { display: flex; align-items: center; gap: 8px; min-width: 0; height: 40px; margin: 0; padding: 0 11px; border: 1px solid var(--border-strong); border-radius: var(--radius-md); background: var(--bg); color: var(--text-muted); transition: border-color var(--transition-fast), box-shadow var(--transition-fast); }
.component-search:focus-within { border-color: var(--primary); box-shadow: 0 0 0 3px var(--primary-light); }
.component-search > span { font-size: 1.25rem; line-height: 1; }
.component-search input { min-width: 0; flex: 1; height: 100%; border: 0; outline: 0; color: var(--text-primary); background: transparent; font-size: .78rem; }
.component-search small { font: 600 .62rem/1 'Cascadia Code', monospace; }
.component-actions { display: flex; align-items: center; gap: 8px; }
.component-actions .btn { min-height: 40px; padding: 8px 13px; }
.component-progress { height: 2px; margin: 0 1px 8px; overflow: hidden; border-radius: 99px; background: var(--border); }
.component-progress i { display: block; height: 100%; background: var(--primary); transition: width .2s ease; }
.component-list { display: flex; flex-direction: column; gap: 8px; min-width: 0; }
.component-empty { padding: 40px 12px; color: var(--text-muted); text-align: center; }
.component-auto { display: inline-flex; align-items: center; gap: 7px; min-height: 40px; padding: 5px 2px; border: 0; color: var(--text-secondary); background: transparent; font-size: .72rem; font-weight: 650; white-space: nowrap; cursor: pointer; }
.component-auto:disabled { opacity: .45; cursor: not-allowed; }
.auto-switch { position: relative; display: inline-flex; align-items: center; width: 32px; height: 18px; padding: 2px; border-radius: 99px; background: var(--border-strong); transition: background var(--transition-fast); }
.auto-switch i { width: 14px; height: 14px; border-radius: 50%; background: var(--surface); box-shadow: var(--shadow-sm); transform: translateX(0); transition: transform var(--transition-fast); }
.component-auto[aria-checked="true"] { color: var(--primary); }
.component-auto[aria-checked="true"] .auto-switch { background: var(--primary); }
.component-auto[aria-checked="true"] .auto-switch i { transform: translateX(14px); }
@media (min-width: 1101px) { .component-list { display: block; column-count: 2; column-gap: 10px; } .component-list :deep(.rc-card) { break-inside: avoid; margin-bottom: 10px; } }
@media (max-width: 980px) {
  .component-toolbar { grid-template-columns: minmax(0, 1fr) auto; }
  .component-categories { grid-column: 1 / -1; }
  .component-search { grid-column: 1; }
}
@media (max-width: 620px) {
  .component-toolbar { grid-template-columns: 1fr; gap: 8px; padding: 7px; }
  .component-search, .component-actions { grid-column: auto; }
  .component-actions { justify-content: space-between; }
  .component-actions .btn { flex: 1; }
  .component-auto { padding-inline: 8px; }
}
</style>
