<script setup lang="ts">
import { onMounted, reactive, ref } from "vue"
import { version as appVersion } from "../../package.json"
import { parseStatusData } from "../registers/codec"
import { applyComponentCatalog, devices, resetComponentCatalog } from "../registers/components"
import { errorMessage } from "../services/protocol"
import { tauriApi } from "../services/tauri"
import { useComponentStatusStore } from "../stores/componentStatus"
import { useSessionStore } from "../stores/session"
import type { ActiveView, ComponentCatalog, ComponentId } from "../types"

const props = defineProps<{ section: ActiveView; deviceId: string }>()
const emit = defineEmits<{ openTool: [mode: "convert" | "4g"] }>()

const session = useSessionStore()
const status = useComponentStatusStore()
const components = devices.filter((device) => device.fileName).map((device) => ({
  id: device.name as ComponentId,
  fileName: device.fileName!,
}))
const importInput = ref<HTMLInputElement>()
const importTarget = ref<ComponentId | null>(null)
const checking = ref(true)
const busy = ref(false)
const overrides = reactive<Record<string, boolean | null | undefined>>({})
const message = ref("")
const messageError = ref(false)

onMounted(async () => {
  if (props.section !== "import-config") return
  await Promise.all(components.map(async ({ id, fileName }) => {
    try {
      overrides[id] = (await tauriApi.readComponentCatalog(fileName)) !== null
    } catch {
      overrides[id] = null
    }
  }))
  checking.value = false
})

function chooseCatalog(id: ComponentId) {
  importTarget.value = id
  importInput.value?.click()
}

async function importCatalog(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ""
  const id = importTarget.value
  if (!file || !id) return

  message.value = ""
  messageError.value = false
  const fileName = components.find((device) => device.id === id)!.fileName
  if (file.name !== fileName) {
    message.value = `${id} 只允许导入 ${fileName}`
    messageError.value = true
    return
  }
  if (status.reading.active || status.inCooldown) {
    message.value = "状态操作进行中，请稍后导入"
    messageError.value = true
    return
  }

  busy.value = true
  try {
    const source = await file.text()
    parseStatusData({}, {}, JSON.parse(source) as ComponentCatalog)
    await tauriApi.writeComponentCatalog(fileName, source)
    applyComponentCatalog(id, source)
    if (session.view.component === id) status.reloadCatalog()
    overrides[id] = true
    message.value = `${id} 已导入覆盖配置`
  } catch (error) {
    message.value = errorMessage(error, "导入配置失败")
    messageError.value = true
  } finally {
    busy.value = false
  }
}

async function clearCatalog(id: ComponentId, fileName: string) {
  if (busy.value || status.reading.active || status.inCooldown) return
  message.value = ""
  messageError.value = false
  busy.value = true
  try {
    await tauriApi.clearComponentCatalog(fileName)
    resetComponentCatalog(id)
    if (session.view.component === id) status.reloadCatalog()
    overrides[id] = false
    message.value = `${id} 已恢复内置配置`
  } catch (error) {
    message.value = errorMessage(error, "清空覆盖配置失败")
    messageError.value = true
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <section class="page settings-page">
    <template v-if="section === 'settings'">
      <h1>设置</h1>
      <div class="settings-list">
        <button type="button" @click="emit('openTool', 'convert')">本地转换</button>
        <button type="button" @click="emit('openTool', '4g')">4G OTA</button>
        <button type="button" @click="session.goTo('import-config')">导入配置</button>
        <button type="button" @click="session.goTo('about')">关于</button>
      </div>
    </template>

    <template v-else>
      <div class="settings-heading">
        <button type="button" class="btn btn-outline btn-sm" @click="session.goTo('settings')">返回</button>
        <h1>{{ section === 'about' ? '关于' : '导入配置' }}</h1>
      </div>

      <div v-if="section === 'import-config'" class="catalog-list">
        <input ref="importInput" type="file" accept=".json,application/json" hidden @change="importCatalog" />
        <div v-for="device in components" :key="device.id" class="catalog-row">
          <div class="catalog-name">
            <strong>{{ device.id }}</strong>
            <span :class="{ overridden: overrides[device.id] === true }">
              {{ checking ? '检查中' : overrides[device.id] === true ? '已覆盖' : overrides[device.id] === false ? '未覆盖' : '读取失败' }}
            </span>
          </div>
          <div class="catalog-actions">
            <button type="button" class="btn btn-outline btn-sm"
              :disabled="checking || busy || status.reading.active || status.inCooldown"
              @click="chooseCatalog(device.id)">导入</button>
            <button type="button" class="btn btn-outline btn-sm"
              :disabled="checking || busy || overrides[device.id] === false || status.reading.active || status.inCooldown"
              @click="clearCatalog(device.id, device.fileName)">清空</button>
          </div>
        </div>
      </div>
      <div v-if="section === 'import-config' && message" class="msg catalog-message"
        :class="messageError ? 'msg-error' : 'msg-success'" :role="messageError ? 'alert' : 'status'">{{ message }}</div>

      <div v-if="section === 'about'" class="settings-detail about-detail">
        <strong>RideX</strong>
        <div><span>版本</span><span>{{ appVersion }}</span></div>
        <div><span>设备 ID</span><span class="device-id">{{ deviceId || '未获取' }}</span></div>
      </div>
    </template>
  </section>
</template>

<style scoped>
.settings-page {
  width: min(100%, 40rem);
  margin: 0 auto;
}

h1 {
  margin: 0 0 1.25rem;
  font-size: 1.35rem;
  line-height: 1.3;
}

.settings-list,
.settings-detail,
.catalog-list {
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
  box-shadow: var(--shadow-sm);
}

.settings-list button {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  min-height: 3.5rem;
  padding: 0.75rem 1rem;
  border: 0;
  border-bottom: 1px solid var(--border);
  color: var(--text-primary);
  background: transparent;
  text-align: left;
  font-weight: 600;
  cursor: pointer;
}

.settings-list button:last-child {
  border-bottom: 0;
}

.settings-list button:hover {
  background: var(--surface-hover);
}

.settings-list button::after {
  width: 0.45rem;
  height: 0.45rem;
  border-top: 1.5px solid var(--text-muted);
  border-right: 1.5px solid var(--text-muted);
  content: "";
  transform: rotate(45deg);
}

.settings-heading {
  display: flex;
  align-items: center;
  gap: 1rem;
  margin-bottom: 1.25rem;
}

.settings-heading h1 {
  margin: 0;
}

.settings-detail {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 1rem;
  padding: 1.25rem;
}

.catalog-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  min-height: 4.25rem;
  padding: 0.75rem 1rem;
  border-bottom: 1px solid var(--border);
}

.catalog-row:last-child {
  border-bottom: 0;
}

.catalog-name {
  display: flex;
  flex-direction: column;
  gap: 0.125rem;
}

.catalog-name strong {
  font-size: 0.95rem;
}

.catalog-name span {
  color: var(--text-muted);
  font-size: 0.75rem;
}

.catalog-name span.overridden {
  color: var(--primary);
  font-weight: 650;
}

.catalog-actions {
  display: flex;
  gap: 0.5rem;
}

.catalog-actions button {
  min-width: 3.5rem;
}

.catalog-message {
  margin-top: 0.75rem;
}

.about-detail {
  gap: 0;
  width: 100%;
}

.about-detail > strong {
  margin-bottom: 0.5rem;
  font-size: 1.2rem;
}

.about-detail > div {
  display: flex;
  justify-content: space-between;
  gap: 1rem;
  width: 100%;
  padding: 0.85rem 0;
  border-top: 1px solid var(--border);
}

.about-detail > div span:first-child {
  color: var(--text-secondary);
}

.device-id {
  overflow-wrap: anywhere;
  text-align: right;
}
</style>
