<script setup>
import { computed, ref } from "vue"
import { errorMessage, fixedHexBytes } from "../services/protocol.js"
import { tauriApi } from "../services/tauri.js"

const inputPath = ref("")
const inputName = ref("")
const manufacturer = ref("00003132")
const deviceType = ref("0000")
const hardwareVersion = ref("0000")
const softwareVersion = ref("0000")
const deviceAddress = ref("FF")
const description = ref("")
const busy = ref(false)
const error = ref("")
const result = ref("")

const outputName = computed(() => {
    const base = inputName.value.replace(/\.bin$/i, "")
    return (base || "firmware") + ".ota"
})

const fileName = (value) => {
    if (!value) return ""
    try {
        value = decodeURIComponent(value)
    } catch {
        // 保留原始路径，文件名仍可正常显示。
    }
    return value.split(/[\\/]/).pop() || value
}

const parseByte = (value, label) => {
    const normalized = value.trim().replace(/^0x/i, "")
    if (!/^[0-9a-f]{1,2}$/i.test(normalized)) {
        throw new Error(label + "必须是 1~2 位 HEX")
    }
    return Number.parseInt(normalized, 16)
}

const chooseBin = async () => {
    error.value = ""
    result.value = ""
    const selected = await tauriApi.pickBin()
    const path = Array.isArray(selected) ? selected[0] : selected
    if (!path) return
    inputPath.value = path
    inputName.value = fileName(path)
}

const convert = async () => {
    error.value = ""
    result.value = ""
    busy.value = true
    try {
        if (!inputPath.value) throw new Error("请先选择 BIN 固件")
        const text = description.value.trim()
        if (!/^[\x00-\x7F]*$/.test(text)) throw new Error("文件描述只能使用 ASCII 字符")
        if (new TextEncoder().encode(text).length > 20) {
            throw new Error("文件描述最多 20 个字节")
        }
        const outputPath = await tauriApi.saveOta(outputName.value)
        if (!outputPath) return
        const info = await tauriApi.otaPack({
            inputPath: inputPath.value,
            outputPath,
            manufacturer: fixedHexBytes(4, "厂家编码").parse(manufacturer.value),
            deviceType: fixedHexBytes(2, "产品类型").parse(deviceType.value),
            hardwareVersion: fixedHexBytes(2, "硬件版本").parse(hardwareVersion.value),
            softwareVersion: fixedHexBytes(2, "软件版本").parse(softwareVersion.value),
            deviceAddress: parseByte(deviceAddress.value, "设备地址"),
            description: text,
        })
        result.value = "已生成 " + fileName(outputPath) + "（" + info.upgradeSize + " 字节）"
    } catch (cause) {
        error.value = errorMessage(cause, "BIN 转 OTA 失败")
    } finally {
        busy.value = false
    }
}
</script>

<template>
  <section class="page bin-ota-page">
    <header class="page-head">
      <div>
        <span class="tool-kicker">FIRMWARE TOOL</span>
        <h1>BIN 转 OTA</h1>
        <p>填写文件头信息，生成可直接用于升级的 OTA 固件</p>
      </div>
    </header>

    <div class="bin-ota-tool">
      <div class="tool-head">
        <span class="tool-kicker">FIRMWARE TOOL</span>
        <strong>BIN 转 OTA</strong>
        <small>填写信息后生成 OTA 固件</small>
      </div>

      <div class="bin-ota-body">
      <div class="bin-file-row">
        <div class="bin-file-copy">
          <span>输入 BIN 固件</span>
          <strong>{{ inputName || '尚未选择文件' }}</strong>
        </div>
        <button type="button" class="btn btn-outline btn-sm" :disabled="busy" @click="chooseBin">
          {{ inputPath ? '更换 BIN' : '选择 BIN' }}
        </button>
      </div>

      <div class="bin-ota-fields">
        <label>
          <span>厂家编码 <small>4 字节 HEX</small></span>
          <input v-model.trim="manufacturer" class="input input-sm input-mono" maxlength="8" placeholder="00003132" spellcheck="false" />
        </label>
        <label>
          <span>产品类型 <small>2 字节 HEX</small></span>
          <input v-model.trim="deviceType" class="input input-sm input-mono" maxlength="4" placeholder="0000" spellcheck="false" />
        </label>
        <label>
          <span>硬件版本 <small>2 字节 HEX</small></span>
          <input v-model.trim="hardwareVersion" class="input input-sm input-mono" maxlength="4" placeholder="0000" spellcheck="false" />
        </label>
        <label>
          <span>软件版本 <small>2 字节 HEX</small></span>
          <input v-model.trim="softwareVersion" class="input input-sm input-mono" maxlength="4" placeholder="0000" spellcheck="false" />
        </label>
        <label>
          <span>设备地址 <small>1 字节 HEX，FF 为不限制</small></span>
          <input v-model.trim="deviceAddress" class="input input-sm input-mono" maxlength="4" placeholder="FF" spellcheck="false" />
        </label>
        <label class="description-field">
          <span>文件描述 <small>ASCII，最多 20 字节</small></span>
          <input v-model="description" class="input input-sm" maxlength="20" placeholder="可选" />
        </label>
      </div>

      <div class="bin-ota-footer">
        <span>输出文件：{{ outputName }} · 自动计算文件 CRC 与升级 CRC</span>
        <button type="button" class="btn btn-primary" :disabled="busy || !inputPath" @click="convert">
          {{ busy ? '正在生成…' : '转换并保存 OTA' }}
        </button>
      </div>
      <div v-if="error" class="msg msg-error">{{ error }}</div>
      <div v-if="result" class="msg msg-success">{{ result }}</div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.bin-ota-page {
  width: 100%;
}

.page-head {
  margin-bottom: 18px;
}

.page-head h1 {
  margin: 6px 0 3px;
  font-size: clamp(1.8rem, 4vw, 2.6rem);
  letter-spacing: -.05em;
}

.page-head p {
  margin: 0;
  color: var(--text-muted);
  font-size: .72rem;
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
  font: 700 .58rem/1 'Cascadia Code', monospace;
  letter-spacing: .12em;
}

.tool-head strong {
  font-size: .9rem;
}

.tool-head small {
  min-width: 0;
  overflow: hidden;
  color: var(--text-muted);
  font-size: .65rem;
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
  font-size: .68rem;
  font-weight: 700;
}

.bin-file-copy strong {
  overflow: hidden;
  color: var(--text-primary);
  font: 700 .75rem/1.2 'Cascadia Code', monospace;
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
  font-size: .55rem;
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
  font-size: .62rem;
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
