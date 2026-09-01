<script setup lang="ts">
import { computed, nextTick, reactive, watch } from "vue"
import { formatContentValue, validateRegisterInput } from "../registers/codec"
import type { ContentDefinition, StatusItem } from "../types"

const FIELD_PREVIEW_LIMIT = 2
const AUTO_EXPAND_LIMIT = 10

const props = withDefaults(
  defineProps<{
    item: StatusItem
    disabled?: boolean
    writing?: boolean
    writeError?: string
    writeRegister: (address: number, contentIndex: number, value: unknown) => Promise<boolean>
    writeCoil: (address: number, value: boolean) => Promise<boolean>
  }>(),
  {
    disabled: false,
    writing: false,
    writeError: "",
  },
)

const card = reactive({
  expanded: false,
  editor: {
    index: null as number | null,
    value: "",
    error: "",
  },
  actionError: "",
})

const addressText = computed(
  () => `${props.item.kind === "coil" ? "C" : "R"}${String(props.item.address).padStart(3, "0")}`,
)
const cardId = computed(() => `status-${props.item.kind}-${props.item.address}`)
const hasOverflow = computed(() => props.item.content.length > FIELD_PREVIEW_LIMIT)
const visibleContent = computed(() =>
  card.expanded
    ? props.item.content
    : props.item.content.slice(0, FIELD_PREVIEW_LIMIT),
)
const canWriteItem = computed(() => props.item.loaded && props.item.writable && !props.disabled)

watch(
  () => props.item.content.length,
  (length) => {
    card.expanded = length > FIELD_PREVIEW_LIMIT && length <= AUTO_EXPAND_LIMIT
  },
  { immediate: true },
)

function fieldName(content: ContentDefinition) {
  return content.label || props.item.label
}

function isEditableContent(content: ContentDefinition) {
  return ["hex", "value", "text", "password", "datetime", "date"].includes(content.kind)
}

function inputType(content: ContentDefinition) {
  if (content.kind === "password") return "password"
  if (content.kind === "datetime") return "datetime-local"
  if (content.kind === "date") return "date"
  if (content.kind === "hex") return "text"
  if (content.codec === "hex_value") return "text"
  if (content.kind === "value") return "number"
  return "text"
}

function inputMode(content: ContentDefinition) {
  if (content.kind === "password") return "numeric"
  if (content.kind === "hex") return "text"
  if (content.codec === "hex_value") return "text"
  if (content.kind === "value") return "decimal"
  return undefined
}

function hasCurrentOption(content: ContentDefinition) {
  return (content.options ?? []).some(
    (option) => Object.is(option.value, content.value) || String(option.value) === String(content.value),
  )
}

function startEdit(index: number, content: ContentDefinition) {
  if (!canWriteItem.value || !isEditableContent(content)) return
  card.editor.index = index
  card.editor.error = ""
  card.actionError = ""
  if (content.kind === "password") card.editor.value = ""
  else if (content.kind === "datetime")
    card.editor.value = String(content.value ?? "").replace(" ", "T")
  else card.editor.value = String(content.value ?? "")
  nextTick(() => document.getElementById(`${cardId.value}-input-${index}`)?.focus())
}

function cancelEdit(restoreFocus = true) {
  const previousIndex = card.editor.index
  card.editor.index = null
  card.editor.value = ""
  card.editor.error = ""
  if (restoreFocus && previousIndex !== null) {
    nextTick(() => document.getElementById(`${cardId.value}-edit-${previousIndex}`)?.focus())
  }
}

async function saveEdit(index: number) {
  const validationError = validateRegisterInput(props.item, index, card.editor.value)
  if (validationError) {
    card.editor.error = validationError
    return
  }

  card.editor.error = ""
  card.actionError = ""
  const saved = await props.writeRegister(props.item.address, index, card.editor.value)
  if (saved) {
    cancelEdit()
    return
  }
  await nextTick()
  card.editor.error = props.writeError || "写入失败"
}

async function writeImmediate(index: number, value: unknown) {
  if (!canWriteItem.value) return
  card.actionError = ""
  const saved =
    props.item.kind === "coil"
      ? await props.writeCoil(props.item.address, Boolean(value))
      : await props.writeRegister(props.item.address, index, value)
  if (!saved) {
    await nextTick()
    card.actionError = props.writeError || "写入失败"
  }
}

watch(
  () => props.writeError,
  (value, previousValue) => {
    if (value) return
    card.actionError = ""
    if (previousValue && card.editor.error === previousValue) card.editor.error = ""
  },
)

function changeSelect(index: number, content: ContentDefinition, event: Event) {
  const value = (event.target as HTMLSelectElement).value
  const option = (content.options ?? []).find(
    (candidate) => String(candidate.value) === value,
  )
  writeImmediate(index, option ? option.value : value)
}

function toggleExpanded() {
  card.expanded = !card.expanded
  cancelEdit(false)
}
</script>

<template>
  <article
    class="rc-card"
    :class="{ 'is-expanded': card.expanded, 'is-writing': writing, 'is-compact': !hasOverflow }"
    :aria-labelledby="`${cardId}-title`"
    :aria-busy="writing"
    @keydown.esc="cancelEdit()"
  >
    <header class="rc-header">
      <div class="rc-identity">
        <div class="rc-meta">
          <strong>{{ addressText }}</strong>
        </div>
        <h3 :id="`${cardId}-title`">{{ item.label }}</h3>
      </div>

      <button
        v-if="hasOverflow"
        class="rc-expand"
        type="button"
        :aria-expanded="card.expanded"
        :aria-controls="`${cardId}-fields`"
        :aria-label="
          card.expanded
            ? `收起${item.label}的完整字段`
            : `展开${item.label}的全部${item.content.length}项字段`
        "
        @click="toggleExpanded"
      >
        <span>{{ card.expanded ? "收起" : `+${item.content.length - FIELD_PREVIEW_LIMIT}` }}</span>
        <svg viewBox="0 0 20 20" aria-hidden="true">
          <path d="m7 4 6 6-6 6" />
        </svg>
      </button>
    </header>

    <div :id="`${cardId}-fields`" class="rc-fields">
      <div
        v-for="(content, visibleIndex) in visibleContent"
        :key="visibleIndex"
        class="rc-field"
        :class="{ 'is-editing': card.editor.index === visibleIndex }"
      >
        <span v-if="content.label" class="rc-field-label">{{ content.label }}</span>

        <select
          v-if="content.kind === 'select' && item.writable && item.loaded"
          class="rc-select"
          :value="content.value"
          :disabled="disabled"
          :aria-label="fieldName(content)"
          @change="changeSelect(visibleIndex, content, $event)"
        >
          <option v-if="!hasCurrentOption(content)" :value="content.value" disabled>
            未知值（{{ content.value }}）
          </option>
          <option
            v-for="option in content.options"
            :key="String(option.value)"
            :value="option.value"
          >
            {{ option.label }}
          </option>
        </select>

        <button
          v-else-if="content.kind === 'switch' && item.writable && item.loaded"
          class="rc-switch"
          type="button"
          role="switch"
          :aria-checked="Boolean(content.value)"
          :aria-label="`${fieldName(content)}：${formatContentValue(content, item)}`"
          :disabled="disabled"
          @click="writeImmediate(visibleIndex, !Boolean(content.value))"
        >
          <span class="rc-switch-track" aria-hidden="true"><i></i></span>
          <strong>{{ formatContentValue(content, item) }}</strong>
        </button>

        <div v-else-if="card.editor.index === visibleIndex" class="rc-editor">
          <div class="rc-editor-row">
            <input
              :id="`${cardId}-input-${visibleIndex}`"
              v-model="card.editor.value"
              class="rc-input"
              :type="inputType(content)"
              :inputmode="inputMode(content)"
              :min="content.min"
              :max="content.max"
              :step="
                content.kind === 'datetime'
                  ? 1
                  : (content.step ?? (content.kind === 'value' ? 'any' : undefined))
              "
              :maxlength="content.kind === 'password' ? 6 : content.maxLength"
              :autocomplete="content.kind === 'password' ? 'new-password' : 'off'"
              :aria-label="`编辑${fieldName(content)}`"
              :aria-invalid="Boolean(card.editor.error)"
              :aria-describedby="card.editor.error ? `${cardId}-error-${visibleIndex}` : undefined"
              :disabled="disabled"
              @keyup.enter="saveEdit(visibleIndex)"
              @keyup.escape.stop="cancelEdit()"
            />
            <span v-if="content.unit" class="rc-unit">{{ content.unit }}</span>
            <button
              class="rc-edit-action is-confirm"
              type="button"
              :disabled="disabled"
              :aria-label="`确认修改${fieldName(content)}`"
              @click="saveEdit(visibleIndex)"
            >
              <svg viewBox="0 0 18 18" aria-hidden="true"><path d="m3 9 4 4 8-9" /></svg>
            </button>
            <button
              class="rc-edit-action"
              type="button"
              :disabled="writing"
              :aria-label="`取消修改${fieldName(content)}`"
              @click="cancelEdit()"
            >
              <svg viewBox="0 0 18 18" aria-hidden="true"><path d="m4 4 10 10M14 4 4 14" /></svg>
            </button>
          </div>
          <small
            v-if="card.editor.error"
            :id="`${cardId}-error-${visibleIndex}`"
            class="rc-error"
            role="alert"
          >
            {{ card.editor.error }}
          </small>
        </div>

        <button
          v-else-if="item.writable && item.loaded && isEditableContent(content)"
          :id="`${cardId}-edit-${visibleIndex}`"
          class="rc-value-button"
          type="button"
          :disabled="disabled"
          :aria-label="`编辑${fieldName(content)}，当前值${formatContentValue(content, item)}`"
          @click="startEdit(visibleIndex, content)"
        >
          <strong>{{ formatContentValue(content, item) }}</strong>
          <svg viewBox="0 0 18 18" aria-hidden="true">
            <path d="m12.8 2.7 2.5 2.5L7 13.5l-3.5 1 1-3.5 8.3-8.3Z" />
          </svg>
        </button>

        <strong v-else class="rc-readout">{{ formatContentValue(content, item) }}</strong>
      </div>

      <small v-if="card.actionError" class="rc-error rc-action-error" role="alert">{{
        card.actionError
      }}</small>
    </div>

    <footer class="rc-raw">
      <span>HEX</span>
      <span class="rc-byte-count">{{ item.bytes }}BYTE</span>
      <code :title="item.loaded ? (item.rawHex ?? '—') : '—'">{{
        item.loaded ? (item.rawHex ?? "—") : "—"
      }}</code>
    </footer>
  </article>
</template>

<style scoped>
.rc-card {
  display: flex;
  flex-direction: column;
  min-width: 0;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  box-shadow: var(--shadow-sm);
}

.rc-card.is-writing {
  cursor: progress;
}

.rc-header {
  display: flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
  min-height: 44px;
  padding: 13px 14px 0;
}

.rc-identity {
  display: flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
  flex: 1;
}

.rc-meta {
  display: flex;
  align-items: baseline;
  gap: 7px;
  flex: 0 0 auto;
  margin: 0;
  font-family: "SF Mono", "Cascadia Code", Consolas, monospace;
  line-height: 1;
}

.rc-meta strong {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 36px;
  padding: 3px 5px;
  border-radius: var(--radius-sm);
  color: var(--primary);
  background: var(--primary-light);
  font-size: 0.72rem;
  letter-spacing: 0.025em;
  text-align: center;
}

.rc-meta span {
  color: var(--text-muted);
  font-size: 0.6rem;
  font-weight: 650;
  letter-spacing: 0.08em;
}

.rc-header h3 {
  min-width: 0;
  margin: 0;
  color: var(--text-secondary);
  font-size: 0.86rem;
  font-weight: 550;
  line-height: 1.3;
  overflow-wrap: anywhere;
}

.rc-expand {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 2px;
  flex: 0 0 auto;
  min-width: 40px;
  min-height: 40px;
  padding: 5px 6px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg);
  color: var(--text-secondary);
  font-size: 0.66rem;
  font-weight: 650;
  cursor: pointer;
}

.rc-expand:hover {
  border-color: var(--border-strong);
  background: var(--surface-hover);
  color: var(--text-primary);
}

.rc-expand svg {
  width: 13px;
  height: 13px;
  fill: none;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.7;
  transition: transform var(--transition-fast);
}

.rc-expand[aria-expanded="true"] svg {
  transform: rotate(90deg);
}

.rc-fields {
  display: flex;
  align-items: stretch;
  flex-wrap: wrap;
  gap: 8px;
  min-width: 0;
  padding: 8px 14px 0;
}

.rc-card.is-compact .rc-fields {
  flex-direction: column;
  gap: 0;
  padding-top: 6px;
}

.rc-field {
  display: flex;
  flex: 1 1 210px;
  flex-direction: column;
  justify-content: center;
  gap: 4px;
  min-width: 0;
  min-height: 40px;
  padding: 0;
}

.rc-card.is-compact .rc-field {
  flex: 0 0 auto;
  min-height: 0;
}

.rc-field-label {
  color: var(--text-muted);
  font-size: 0.69rem;
  line-height: 1.25;
  overflow-wrap: anywhere;
}

.rc-readout,
.rc-value-button strong,
.rc-switch strong {
  min-width: 0;
  color: var(--primary);
  font-family: "SF Mono", "Cascadia Code", Consolas, monospace;
  font-size: 0.92rem;
  font-weight: 700;
  line-height: 1.3;
  overflow-wrap: anywhere;
}

.rc-readout {
  display: block;
}

.rc-value-button {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  width: 100%;
  min-height: 40px;
  margin: 0;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--text-secondary);
  text-align: left;
  cursor: pointer;
}

.rc-value-button:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.rc-value-button svg {
  flex: 0 0 auto;
  width: 17px;
  height: 17px;
  box-sizing: content-box;
  padding: 10px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  fill: none;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.5;
}

.rc-switch {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  min-height: 40px;
  margin: 0;
  padding: 4px 0;
  border: 0;
  background: transparent;
  text-align: left;
  cursor: pointer;
}

.rc-switch:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.rc-switch-track {
  position: relative;
  flex: 0 0 auto;
  width: 34px;
  height: 20px;
  border: 1px solid var(--border-strong);
  border-radius: 999px;
  background: var(--gray);
  transition:
    background var(--transition-fast),
    border-color var(--transition-fast);
}

.rc-switch-track i {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: var(--surface);
  box-shadow: var(--shadow-sm);
  transition: transform var(--transition-fast);
}

.rc-switch[aria-checked="true"] .rc-switch-track {
  border-color: var(--primary);
  background: var(--primary);
}

.rc-switch[aria-checked="true"] .rc-switch-track i {
  transform: translateX(14px);
}

.rc-select,
.rc-input {
  width: 100%;
  min-width: 0;
  min-height: 40px;
  padding: 7px 9px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  outline: 0;
  background: var(--bg);
  color: var(--text-primary);
  font-size: 0.8rem;
}

.rc-select:focus,
.rc-input:focus {
  border-color: var(--primary);
  box-shadow: 0 0 0 3px var(--primary-light);
}

.rc-select:disabled,
.rc-input:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.rc-editor {
  min-width: 0;
}

.rc-editor-row {
  display: flex;
  align-items: center;
  gap: 5px;
  min-width: 0;
}

.rc-input {
  flex: 1;
}

.rc-input[type="number"] {
  -moz-appearance: textfield;
  appearance: textfield;
}

.rc-input[type="number"]::-webkit-inner-spin-button,
.rc-input[type="number"]::-webkit-outer-spin-button {
  margin: 0;
  -webkit-appearance: none;
}

.rc-unit {
  flex: 0 0 auto;
  color: var(--text-muted);
  font-size: 0.7rem;
}

.rc-edit-action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: 0 0 40px;
  width: 40px;
  height: 40px;
  padding: 0;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface);
  color: var(--text-secondary);
  cursor: pointer;
}

.rc-edit-action.is-confirm {
  border-color: var(--primary);
  color: var(--primary);
}

.rc-edit-action:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.rc-edit-action svg {
  width: 17px;
  height: 17px;
  fill: none;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.8;
}

.rc-error {
  display: block;
  margin-top: 4px;
  color: var(--danger);
  font-size: 0.68rem;
  line-height: 1.3;
  overflow-wrap: anywhere;
}

.rc-action-error {
  flex: 1 0 100%;
  padding: 0 0 7px;
}

.rc-raw {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  align-self: flex-start;
  max-width: calc(100% - 28px);
  min-width: 0;
  margin: 3px 14px 13px;
  padding: 4px 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: color-mix(in srgb, var(--bg) 72%, var(--surface));
}

.rc-raw span {
  flex: 0 0 auto;
  color: var(--text-muted);
  font:
    750 0.62rem/1.4 "SF Mono",
    "Cascadia Code",
    Consolas,
    monospace;
  letter-spacing: 0.08em;
}

.rc-raw .rc-byte-count {
  order: 1;
  margin-left: 2px;
  color: var(--text-muted);
  font-size: 0.58rem;
  letter-spacing: 0.04em;
}

.rc-raw code {
  flex: 0 1 auto;
  min-width: 0;
  overflow-x: auto;
  overflow-y: hidden;
  color: var(--primary);
  font:
    650 0.72rem/1.4 "SF Mono",
    "Cascadia Code",
    Consolas,
    monospace;
  scrollbar-width: thin;
  user-select: all;
  white-space: nowrap;
}

.rc-card.is-expanded .rc-header {
  border-bottom: 1px solid var(--border);
}

.rc-card.is-expanded .rc-field {
  flex-basis: 220px;
}

@media (max-width: 900px) {
  .rc-card {
    display: flex;
  }

  .rc-header {
    align-items: center;
    min-height: 44px;
    padding: 13px 14px 0;
    border: 0;
  }

  .rc-header h3 {
    font-size: 0.8rem;
  }

  .rc-expand {
    min-width: 40px;
    width: 40px;
    padding: 4px;
  }

  .rc-expand svg {
    display: none;
  }

  .rc-fields {
    flex-direction: column;
  }

  .rc-field {
    flex: 1 1 auto;
    min-height: 40px;
    padding: 0;
  }

  .rc-field.is-editing {
    flex-basis: 100%;
  }

  .rc-card.is-expanded .rc-header {
    align-items: center;
    min-height: 44px;
  }

  .rc-card.is-expanded .rc-fields {
    flex-direction: row;
  }

  .rc-card.is-expanded .rc-field {
    flex-basis: 150px;
    min-height: 58px;
  }

  .rc-card.is-expanded .rc-field.is-editing {
    flex-basis: 100%;
  }

  .rc-editor-row {
    flex-wrap: wrap;
  }

  .rc-input {
    flex-basis: calc(100% - 92px);
  }

  .rc-unit {
    order: 2;
  }
}

@media (prefers-reduced-motion: reduce) {
  .rc-expand svg,
  .rc-switch-track,
  .rc-switch-track i {
    transition: none;
  }
}
</style>
