<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from "vue"

const props = defineProps({
  categories: { type: Array, default: () => [] },
  activeCategoryId: { type: String, default: "" },
})

const emit = defineEmits(["select-category"])
const root = ref(null)
const open = ref(false)
const selectedCategory = computed(() => props.categories.find((category) => category.id === props.activeCategoryId) ?? props.categories[0])

function toggleMenu() {
  open.value = !open.value
}

function selectCategory(category) {
  emit("select-category", category.id)
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
  <div ref="root" class="category-picker">
    <button
      class="category-trigger"
      type="button"
      aria-haspopup="listbox"
      :aria-expanded="open"
      aria-label="切换状态分类"
      @click="toggleMenu"
    >
      <strong>{{ selectedCategory?.name ?? "选择分类" }}</strong>
      <svg :class="{ open }" viewBox="0 0 16 16" aria-hidden="true">
        <path d="m3.5 6 4.5 4 4.5-4" />
      </svg>
    </button>

    <div v-if="open" class="category-menu" role="listbox" aria-label="选择状态分类">
      <button
        v-for="category in categories"
        :key="category.id"
        class="category-option"
        :class="{ active: category.id === activeCategoryId }"
        type="button"
        role="option"
        :aria-selected="category.id === activeCategoryId"
        @click="selectCategory(category)"
      >
        <span>{{ category.name }}</span>
        <svg v-if="category.id === activeCategoryId" viewBox="0 0 18 18" aria-hidden="true">
          <path d="m3.5 9.5 3.2 3.2 7.8-8" />
        </svg>
      </button>
    </div>
  </div>
</template>

<style scoped>
.category-picker {
  position: relative;
  width: 100%;
  min-width: 0;
}

.category-trigger {
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

.category-trigger:hover,
.category-trigger[aria-expanded="true"] {
  border-color: var(--primary);
  box-shadow: 0 0 0 3px var(--primary-light);
}

.category-trigger strong {
  min-width: 0;
  overflow: hidden;
  font-size: .76rem;
  font-weight: 650;
  line-height: 1.2;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.category-trigger svg {
  width: 16px;
  height: 16px;
  flex: 0 0 auto;
  fill: none;
  stroke: var(--text-secondary);
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.6;
  transition: transform var(--transition-fast), stroke var(--transition-fast);
}

.category-trigger svg.open {
  transform: rotate(180deg);
  stroke: var(--primary);
}

.category-menu {
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

.category-option {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
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

.category-option:hover { background: var(--surface-hover); color: var(--text-primary); }
.category-option.active { background: var(--primary-light); color: var(--primary); font-weight: 700; }

.category-option > span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.category-option > svg {
  width: 16px;
  height: 16px;
  flex: 0 0 auto;
  fill: none;
  stroke: var(--primary);
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.8;
}

@media (prefers-reduced-motion: reduce) {
  .category-trigger,
  .category-trigger svg,
  .category-option { transition: none; }
}
</style>
