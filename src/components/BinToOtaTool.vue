<script setup lang="ts">
import { computed, reactive, ref } from "vue"
import { errorMessage, parseHexBytes } from "../services/protocol"
import { tauriApi } from "../services/tauri"
import { useSessionStore } from "../stores/session"
import { displayFileName } from "../utils/displayFileName"
import FourGOtaPanel from "./FourGOtaPanel.vue"
import { version as appVersion } from "../../package.json"

defineProps<{ deviceId: string }>()

const session = useSessionStore()
const mode = ref<"convert" | "4g">("convert")

const tool = reactive({
  inputPath: "",
  manufacturer: "00003132",
  deviceType: "0000",
  hardwareVersion: "0000",
  softwareVersion: "0000",
  deviceAddress: "02",
  description: "",
  busy: false,
  error: "",
  result: "",
})

const displayVersion = (value: string) => {
  const hex = value.trim()
  if (!/^[\dA-Fa-f]{4}$/.test(hex)) return hex
  return `${Number.parseInt(hex.slice(2, 4), 16)}.${Number.parseInt(hex.slice(0, 2), 16)}`
}

const outputName = computed(() => {
  return `${displayVersion(tool.hardwareVersion)}.${displayVersion(tool.softwareVersion)}.ota`
})

const chooseFirmware = async () => {
  tool.error = ""
  tool.result = ""
  const path = await tauriApi.pickFirmwareSource()
  if (!path) return
  tool.inputPath = path
}

const leaveTool = () => {
  session.goTo(session.authenticated ? "status" : "search")
}

const convert = async () => {
  tool.error = ""
  tool.result = ""
  tool.busy = true
  try {
    if (!tool.inputPath) throw new Error("请先选择固件文件")
    const outputPath = await tauriApi.saveOta(outputName.value)
    if (!outputPath) return
    await tauriApi.otaPack({
      inputPath: tool.inputPath,
      outputPath,
      request: {
        manufacturer: parseHexBytes(tool.manufacturer, 4, "厂家编码"),
        deviceType: parseHexBytes(tool.deviceType, 2, "产品类型"),
        hardwareVersion: parseHexBytes(tool.hardwareVersion, 2, "硬件版本"),
        softwareVersion: parseHexBytes(tool.softwareVersion, 2, "软件版本"),
        deviceAddress: parseHexBytes(tool.deviceAddress, 1, "设备地址")[0],
        description: tool.description.trim(),
      },
    })
    tool.result = "已生成 " + displayFileName(outputPath)
  } catch (cause) {
    tool.error = errorMessage(cause, "转 OTA 失败")
  } finally {
    tool.busy = false
  }
}
</script>

<template>
  <section class="page bin-ota-page">
    <div class="tool-toolbar">
      <button type="button" class="btn btn-outline btn-sm page-back" @click="leaveTool">返回</button>
      <div class="tool-tabs" role="tablist" aria-label="固件工具模式">
        <button type="button" role="tab" :aria-selected="mode === 'convert'" :class="{ active: mode === 'convert' }"
          @click="mode = 'convert'">
          本地转换
        </button>
        <button type="button" role="tab" :aria-selected="mode === '4g'" :class="{ active: mode === '4g' }"
          @click="mode = '4g'">
          4G OTA
        </button>
      </div>
      <span class="tool-version">Ride{{ appVersion }}</span>
    </div>

    <div v-show="mode === 'convert'" class="bin-ota-tool">
      <div class="bin-ota-body">
        <div class="bin-file-row">
          <div class="bin-file-copy">
            <strong>{{ displayFileName(tool.inputPath) || "尚未选择文件" }}</strong>
          </div>
          <button type="button" class="btn btn-outline btn-sm" :disabled="tool.busy" @click="chooseFirmware">
            {{ tool.inputPath ? "更换固件" : "选择固件" }}
          </button>
        </div>

        <div class="bin-ota-fields">
          <label>
            <span>厂家编码 <small>4 字节 HEX</small></span>
            <input v-model.trim="tool.manufacturer" class="input input-sm input-mono" maxlength="8"
              placeholder="00003132" spellcheck="false" />
          </label>
          <label>
            <span>产品类型 <small>2 字节 HEX</small></span>
            <input v-model.trim="tool.deviceType" class="input input-sm input-mono" maxlength="4" placeholder="0000"
              spellcheck="false" />
          </label>
          <label>
            <span>硬件版本 <small>2 字节 HEX</small></span>
            <input v-model.trim="tool.hardwareVersion" class="input input-sm input-mono" maxlength="4"
              placeholder="0000" spellcheck="false" />
          </label>
          <label>
            <span>软件版本 <small>2 字节 HEX</small></span>
            <input v-model.trim="tool.softwareVersion" class="input input-sm input-mono" maxlength="4"
              placeholder="0000" spellcheck="false" />
          </label>
          <label>
            <span>设备地址 <small>1 字节 HEX</small></span>
            <input v-model.trim="tool.deviceAddress" class="input input-sm input-mono" maxlength="4" placeholder="FF"
              spellcheck="false" />
          </label>
          <label class="description-field">
            <span>文件描述 <small>ASCII，最多 20 字节</small></span>
            <input v-model="tool.description" class="input input-sm" maxlength="20" placeholder="可选" />
          </label>
        </div>

        <div class="bin-ota-footer">
          <span>输出：{{ outputName }}</span>
          <button type="button" class="btn btn-primary" :disabled="tool.busy || !tool.inputPath" @click="convert">
            {{ tool.busy ? "正在生成…" : "转换并保存 OTA" }}
          </button>
        </div>
        <div v-if="tool.error" class="msg msg-error">{{ tool.error }}</div>
        <div v-if="tool.result" class="msg msg-success">{{ tool.result }}</div>
      </div>
    </div>
    <FourGOtaPanel v-show="mode === '4g'" :device-id="deviceId" />
  </section>
</template>

<style scoped>
.bin-ota-page {
  width: 100%;
  min-height: 0;
  display: flex;
  flex: 1;
  flex-direction: column;
}

.tool-toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  margin-bottom: 0.75rem;
}

.tool-version {
  margin-left: auto;
  padding: 0.3rem 0.7rem;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: rgba(0, 143, 149, 0.08);
  color: var(--primary);
  font-size: 0.75rem;
  font-weight: 700;
  white-space: nowrap;
}

.page-back {
  flex: 0 0 auto;
}

.bin-ota-tool {
  min-height: 0;
  display: flex;
  flex: 1;
  flex-direction: column;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  box-shadow: var(--shadow-sm);
}

.tool-tabs {
  display: inline-flex;
  gap: 0.1875rem;
  margin-left: auto;
  padding: 0.1875rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: rgba(11, 24, 38, 0.035);
}

.tool-tabs button {
  padding: 0.375rem 0.875rem;
  border: 0;
  border-radius: 0.375rem;
  color: var(--text-muted);
  background: transparent;
  font-size: 0.72rem;
  font-weight: 700;
  cursor: pointer;
}

.tool-tabs button.active {
  color: var(--primary);
  background: var(--surface);
  box-shadow: var(--shadow-sm);
}

.bin-ota-body {
  display: grid;
  flex: 1;
  grid-template-rows: auto minmax(0, 1fr) auto;
  gap: 0.75rem;
  padding: 1.0625rem;
}

.bin-file-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  padding: 0.75rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg);
}

.bin-file-copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.bin-file-copy>span,
.bin-ota-fields label>span {
  color: var(--text-secondary);
  font-size: 0.68rem;
  font-weight: 700;
}

.bin-file-copy strong {
  overflow: hidden;
  color: var(--text-primary);
  font:
    700 0.75rem/1.2 "Cascadia Code",
    monospace;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.bin-ota-fields {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  align-content: center;
  gap: 0.625rem;
}

.bin-ota-fields label {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 0.3125rem;
}

.bin-ota-fields label>span {
  display: flex;
  justify-content: space-between;
  gap: 0.3125rem;
}

.bin-ota-fields label>span small {
  color: var(--text-muted);
  font-size: 0.55rem;
  font-weight: 500;
}

.description-field {
  grid-column: span 2;
}

.bin-ota-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  padding-top: 0.125rem;
}

.bin-ota-footer>span {
  min-width: 0;
  overflow: hidden;
  color: var(--text-muted);
  font-size: 0.62rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@media (max-width: 760px) {
  .bin-ota-fields {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .description-field {
    grid-column: 1 / -1;
  }
}

@media (max-width: 480px) {

  .bin-ota-body {
    padding-left: 0.75rem;
    padding-right: 0.75rem;
  }

  .bin-file-row,
  .bin-ota-footer {
    align-items: stretch;
    flex-direction: column;
  }

  .bin-ota-footer>span {
    white-space: normal;
  }

  .bin-ota-footer .btn {
    width: 100%;
  }
}
</style>
