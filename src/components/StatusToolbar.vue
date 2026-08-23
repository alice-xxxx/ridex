<script setup>
import CategoryPicker from "./CategoryPicker.vue"
import ComponentPicker from "./ComponentPicker.vue"

defineProps({
  categories: { type: Array, default: () => [] },
  activeCategoryId: { type: String, default: "" },
  query: { type: String, default: "" },
  refreshing: Boolean,
  progressText: { type: String, default: "读取中" },
  refreshDisabled: Boolean,
  autoRefresh: Boolean,
  autoDisabled: Boolean,
})

const emit = defineEmits(["select-category", "update:query", "refresh", "toggle-auto"])

function updateQuery(event) {
  emit("update:query", event.target.value)
}
</script>

<template>
  <div class="status-toolbar">
    <ComponentPicker class="status-component-picker" />

    <nav class="status-categories" aria-label="状态分类">
      <button
        v-for="category in categories"
        :key="category.id"
        class="status-category"
        :class="{ active: activeCategoryId === category.id }"
        type="button"
        :aria-pressed="activeCategoryId === category.id"
        @click="emit('select-category', category.id)"
      >
        <span>{{ category.name }}</span>
      </button>
    </nav>

    <CategoryPicker
      class="status-category-picker"
      :categories="categories"
      :active-category-id="activeCategoryId"
      @select-category="emit('select-category', $event)"
    />

    <div class="status-search-actions">
      <label class="status-search">
        <svg viewBox="0 0 20 20" aria-hidden="true">
          <circle cx="8.5" cy="8.5" r="5.5" />
          <path d="m13 13 4 4" />
        </svg>
        <input
          :value="query"
          type="search"
          placeholder="搜索"
          aria-label="搜索"
          @input="updateQuery"
        />
      </label>

      <button
        class="btn btn-outline status-refresh"
        type="button"
        :disabled="refreshDisabled"
        :aria-busy="refreshing"
        :aria-label="refreshing ? progressText : '立即读取状态'"
        @click="emit('refresh')"
      >
        <span v-if="refreshing" class="spinner" aria-hidden="true"></span>
        <svg v-else viewBox="0 0 20 20" aria-hidden="true">
          <path d="M16.5 7A7 7 0 1 0 17 11" />
          <path d="M16.5 3v4h-4" />
        </svg>
        <span class="status-refresh-text">{{ refreshing ? progressText : "读取" }}</span>
      </button>

      <button
        class="status-auto"
        type="button"
        role="switch"
        :aria-checked="autoRefresh"
        :disabled="autoDisabled"
        :aria-label="autoRefresh ? '关闭自动刷新' : '开启自动刷新'"
        @click="emit('toggle-auto')"
      >
        <span class="status-auto-track" aria-hidden="true"><i></i></span>
        <span>自动</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
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

.status-categories {
  display: flex;
  align-items: stretch;
  gap: 2px;
  min-width: 0;
  overflow-x: auto;
  scrollbar-width: none;
}

:deep(.status-component-picker),
:deep(.status-category-picker) { display: none; }

.status-categories::-webkit-scrollbar { display: none; }

.status-category {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  min-height: 36px;
  padding: 6px 9px;
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-secondary);
  font-size: .74rem;
  cursor: pointer;
  white-space: nowrap;
}

.status-category:hover { background: var(--surface-hover); }
.status-category.active { background: var(--primary-light); color: var(--primary); font-weight: 650; }

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
  background: var(--surface);
  color: var(--text-muted);
  transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
}

.status-search-actions { display: contents; }

.status-search:focus-within { border-color: var(--primary); box-shadow: 0 0 0 3px var(--primary-light); }
.status-search svg { flex: 0 0 auto; width: 16px; height: 16px; fill: none; stroke: currentColor; stroke-width: 1.6; }
.status-search input { min-width: 0; width: 100%; height: 100%; padding: 0 7px; border: 0; outline: 0; background: transparent; color: var(--text-primary); font-size: .76rem; }

.status-refresh { min-width: 78px; min-height: 36px; padding: 6px 10px; }
.status-refresh svg { width: 17px; height: 17px; fill: none; stroke: currentColor; stroke-linecap: round; stroke-linejoin: round; stroke-width: 1.7; }

.status-auto {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  min-height: 36px;
  padding: 5px 8px;
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-secondary);
  font-size: .72rem;
  font-weight: 600;
  cursor: pointer;
  white-space: nowrap;
}

.status-auto:disabled { opacity: .45; cursor: not-allowed; }
.status-auto-track { position: relative; width: 28px; height: 16px; border: 1px solid var(--border-strong); border-radius: 999px; background: var(--gray); transition: background var(--transition-fast), border-color var(--transition-fast); }
.status-auto-track i { position: absolute; top: 2px; left: 2px; width: 10px; height: 10px; border-radius: 50%; background: var(--surface); box-shadow: var(--shadow-sm); transition: transform var(--transition-fast); }
.status-auto[aria-checked="true"] .status-auto-track { border-color: var(--primary); background: var(--primary); }
.status-auto[aria-checked="true"] .status-auto-track i { transform: translateX(12px); }

@media (max-width: 1100px) {
  .status-toolbar { flex-wrap: wrap; }
  .status-categories { flex: 1 0 100%; }
}

@media (max-width: 767px) {
  .status-toolbar { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 6px; padding: 6px; }
  .status-categories { display: none; }
  :deep(.status-component-picker),
  :deep(.status-category-picker) { display: block; grid-row: 1; min-width: 0; }
  :deep(.status-component-picker) { width: 100%; }
  .status-search-actions { display: flex; grid-column: 1 / -1; grid-row: 2; align-items: center; gap: 6px; min-width: 0; }
  .status-search { flex: 1 1 auto; min-width: 0; height: 40px; }
  .status-refresh { flex: 0 0 44px; min-width: 44px; min-height: 40px; padding: 0; }
  .status-refresh-text { display: none; }
  .status-auto { flex: 0 0 auto; min-height: 40px; padding: 4px 8px; }
  .status-auto > span:last-child { display: inline; }
}
</style>
