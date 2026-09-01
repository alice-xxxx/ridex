<script setup lang="ts">
import { computed, reactive } from "vue"
import { errorMessage, parseHexBytes } from "../services/protocol"
import { tauriApi } from "../services/tauri"
import { useSessionStore } from "../stores/session"

const session = useSessionStore()

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

const fileName = (value: string) => {
  if (!value) return ""
  try {
    value = decodeURIComponent(value)
  } catch {
    // 保留原始路径，文件名仍可正常显示。
  }
  return value.split(/[\\/]/).pop() || value
}

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
    tool.result = "已生成 " + fileName(outputPath)
  } catch (cause) {
    tool.error = errorMessage(cause, "转 OTA 失败")
  } finally {
    tool.busy = false
  }
}
</script>

<template>
  <section class="page bin-ota-page">
    <header class="page-head">
      <div>
        <span class="tool-kicker">FIRMWARE TOOL</span>
        <h1>固件转 OTA</h1>
        <p>填写文件头信息，生成可直接用于升级的 OTA 固件</p>
      </div>
      <button type="button" class="btn btn-outline btn-sm page-back" @click="leaveTool">返回</button>
    </header>

    <div class="bin-ota-tool">
      <div class="tool-head">
        <span class="tool-kicker">FIRMWARE TOOL</span>
        <strong>固件转 OTA</strong>
        <small>填写信息后生成 OTA 固件</small>
      </div>

      <div class="bin-ota-body">
        <div class="bin-file-row">
          <div class="bin-file-copy">
            <span>输入固件（BIN / HEX / SREC / TI-TXT / VMEM / ELF / AXF）</span>
            <strong>{{ fileName(tool.inputPath) || "尚未选择文件" }}</strong>
          </div>
          <button type="button" class="btn btn-outline btn-sm" :disabled="tool.busy" @click="chooseFirmware">
            {{ tool.inputPath ? "更换固件" : "选择固件" }}
          </button>
        </div>

        <div class="bin-ota-fields">
          <label>
            <span>厂家编码 <small>4 字节 HEX</small></span>
            <input
              v-model.trim="tool.manufacturer"
              class="input input-sm input-mono"
              maxlength="8"
              placeholder="00003132"
              spellcheck="false"
            />
          </label>
          <label>
            <span>产品类型 <small>2 字节 HEX</small></span>
            <input
              v-model.trim="tool.deviceType"
              class="input input-sm input-mono"
              maxlength="4"
              placeholder="0000"
              spellcheck="false"
            />
          </label>
          <label>
            <span>硬件版本 <small>2 字节 HEX</small></span>
            <input
              v-model.trim="tool.hardwareVersion"
              class="input input-sm input-mono"
              maxlength="4"
              placeholder="0000"
              spellcheck="false"
            />
          </label>
          <label>
            <span>软件版本 <small>2 字节 HEX</small></span>
            <input
              v-model.trim="tool.softwareVersion"
              class="input input-sm input-mono"
              maxlength="4"
              placeholder="0000"
              spellcheck="false"
            />
          </label>
          <label>
            <span>设备地址 <small>1 字节 HEX，FF 为不限制</small></span>
            <input
              v-model.trim="tool.deviceAddress"
              class="input input-sm input-mono"
              maxlength="4"
              placeholder="FF"
              spellcheck="false"
            />
          </label>
          <label class="description-field">
            <span>文件描述 <small>ASCII，最多 20 字节</small></span>
            <input
              v-model="tool.description"
              class="input input-sm"
              maxlength="20"
              placeholder="可选"
            />
          </label>
        </div>

        <div class="bin-ota-footer">
          <span>输出文件：{{ outputName }} · 自动计算文件 CRC 与升级 CRC</span>
          <button
            type="button"
            class="btn btn-primary"
            :disabled="tool.busy || !tool.inputPath"
            @click="convert"
          >
            {{ tool.busy ? "正在生成…" : "转换并保存 OTA" }}
          </button>
        </div>
        <div v-if="tool.error" class="msg msg-error">{{ tool.error }}</div>
        <div v-if="tool.result" class="msg msg-success">{{ tool.result }}</div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.bin-ota-page {
  width: 100%;
}

.page-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 18px;
}

.page-back {
  flex: 0 0 auto;
}

.page-head h1 {
  margin: 6px 0 3px;
  font-size: clamp(1.8rem, 4vw, 2.6rem);
  letter-spacing: -0.05em;
}

.page-head p {
  margin: 0;
  color: var(--text-muted);
  font-size: 0.72rem;
}

.bin-ota-tool {
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  box-shadow: var(--shadow-sm);
}

.tool-head {
  display: grid;
  grid-template-columns: auto auto minmax(0, 1fr);
  align-items: baseline;
  gap: 9px;
  padding: 15px 17px;
  color: var(--text-primary);
}

.tool-kicker {
  color: var(--primary);
  font:
    700 0.58rem/1 "Cascadia Code",
    monospace;
  letter-spacing: 0.12em;
}

.tool-head strong {
  font-size: 0.9rem;
}

.tool-head small {
  min-width: 0;
  overflow: hidden;
  color: var(--text-muted);
  font-size: 0.65rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.bin-ota-body {
  display: grid;
  gap: 12px;
  padding: 0 17px 17px;
}

.bin-file-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg);
}

.bin-file-copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.bin-file-copy > span,
.bin-ota-fields label > span {
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
  gap: 10px;
}

.bin-ota-fields label {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 5px;
}

.bin-ota-fields label > span {
  display: flex;
  justify-content: space-between;
  gap: 5px;
}

.bin-ota-fields label > span small {
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
  gap: 12px;
  padding-top: 2px;
}

.bin-ota-footer > span {
  min-width: 0;
  overflow: hidden;
  color: var(--text-muted);
  font-size: 0.62rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@media (max-width: 760px) {
  .tool-head {
    grid-template-columns: auto minmax(0, 1fr) auto;
  }

  .tool-kicker {
    grid-column: 1 / -1;
  }

  .bin-ota-fields {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .description-field {
    grid-column: 1 / -1;
  }
}

@media (max-width: 480px) {
  .tool-head,
  .bin-ota-body {
    padding-left: 12px;
    padding-right: 12px;
  }

  .tool-head {
    grid-template-columns: minmax(0, 1fr) auto;
  }

  .tool-kicker {
    grid-column: 1 / -1;
  }

  .tool-head small {
    grid-column: 1 / -1;
    grid-row: 3;
  }

  .bin-file-row,
  .bin-ota-footer {
    align-items: stretch;
    flex-direction: column;
  }

  .bin-ota-footer > span {
    white-space: normal;
  }

  .bin-ota-footer .btn {
    width: 100%;
  }
}
</style>
