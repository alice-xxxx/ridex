import { computed, ref, watch } from "vue"
import { defineStore } from "pinia"
import { useSessionStore } from "./session.js"
import { tauriApi } from "../services/tauri.js"
import {
    coilStates, errorMessage, fixedHexBytes, hexWords, isOtaCancelled,
} from "../services/protocol.js"

const txState = () => ({ request: "", raw: "", data: "", error: "" })
const formatHex = (bytes) => bytes.map((byte) => byte.toString(16).padStart(2, "0")).join(" ")
const compactHex = (bytes) => bytes.map((byte) => byte.toString(16).padStart(2, "0").toUpperCase()).join("")

export const useTerminalStore = defineStore("terminal", () => {
    const session = useSessionStore()
    const supportsOta = computed(() => session.supportsOta)
    const writing = ref(false)
    const commLog = ref([])
    let nextLogId = 1

    const readDev = ref(1)
    const readStart = ref(0)
    const readCount = ref(1)
    const readRegResp = ref(txState())
    const writeDev = ref(1)
    const writeStart = ref(0)
    const writeValues = ref("")
    const writeRegResp = ref(txState())
    const readCoilDev = ref(1)
    const readCoilStart = ref(0)
    const readCoilCount = ref(1)
    const readCoilResp = ref(txState())
    const writeCoilDev = ref(1)
    const writeCoilStart = ref(0)
    const writeCoilStates = ref("")
    const writeCoilResp = ref(txState())

    const otaFilePath = ref("")
    const otaFileName = ref("")
    const otaDev = ref(2)
    const otaManufacturer = ref("00003132")
    const otaHwVersion = ref("")
    const otaSwVersion = ref("")
    const fileKind = ref("binary")
    const otaInfo = ref(null)
    const cycles = ref(1)
    const otaActive = ref(false)
    const otaCancelling = ref(false)
    const otaProgress = ref(0)
    const otaElapsedSeconds = ref(0)
    const otaError = ref("")
    const otaResult = ref(null)
    let otaStartedAt = 0
    let otaClock

    function appendLog(label, response) {
        commLog.value.unshift({
            id: nextLogId++,
            time: new Date().toLocaleTimeString("zh-CN", { hour12: false }),
            type: response.error ? "error" : "success",
            label,
            ...response,
        })
        if (commLog.value.length > 50) commLog.value.length = 50
    }

    async function tx(label, response, makeRequest, call = tauriApi.modbusTx) {
        if (writing.value) return
        if (session.statusOperationActive) {
            const result = { ...txState(), error: "车辆状态操作尚未完成，请稍后再试" }
            response.value = result
            appendLog(label, result)
            return
        }
        if (otaActive.value) {
            const result = { ...txState(), error: "OTA 更新中，其他协议操作已暂停" }
            response.value = result
            appendLog(label, result)
            return
        }
        writing.value = true
        response.value = txState()
        try {
            const result = await call(makeRequest())
            const formatted = {
                request: formatHex(result.request),
                raw: formatHex(result.response),
                data: formatHex(result.data),
                error: "",
            }
            response.value = formatted
            appendLog(label, formatted)
        } catch (error) {
            const formatted = { ...txState(), error: errorMessage(error) }
            response.value = formatted
            appendLog(label, formatted)
            session.handleDisconnected(error)
        } finally {
            writing.value = false
        }
    }

    const sendReadRegisters = () => tx("读寄存器", readRegResp, () => ({
        operation: "readRegisters",
        device: readDev.value,
        start: readStart.value,
        count: readCount.value,
    }))
    const sendWriteRegisters = () => tx("写寄存器", writeRegResp, () => ({
        operation: "writeRegisters",
        device: writeDev.value,
        start: writeStart.value,
        values: hexWords.parse(writeValues.value),
    }))
    const sendReadCoils = () => tx("读线圈", readCoilResp, () => ({
        operation: "readCoils",
        device: readCoilDev.value,
        start: readCoilStart.value,
        count: readCoilCount.value,
    }))
    const sendWriteCoils = () => tx("写线圈", writeCoilResp, () => ({
        operation: "writeCoils",
        device: writeCoilDev.value,
        start: writeCoilStart.value,
        values: coilStates.parse(writeCoilStates.value),
    }))

    function clearResponses() {
        readRegResp.value = txState()
        writeRegResp.value = txState()
        readCoilResp.value = txState()
        writeCoilResp.value = txState()
    }

    function clearLogs() {
        commLog.value = []
        clearResponses()
    }

    function firmwareFileName(value) {
        let decoded = String(value ?? "")
        for (let attempt = 0; attempt < 2; attempt += 1) {
            try {
                const next = decodeURIComponent(decoded)
                if (next === decoded) break
                decoded = next
            } catch {
                break
            }
        }
        const normalized = decoded.replace(/\\/g, "/")
        return normalized.split("/").filter(Boolean).at(-1) || decoded
    }

    async function selectFirmware() {
        otaError.value = ""
        otaResult.value = null
        try {
            const selected = await tauriApi.pickFirmware()
            if (!selected) return
            otaFilePath.value = selected
            otaFileName.value = firmwareFileName(selected)
            fileKind.value = selected.toLowerCase().endsWith(".ota") ? "packaged" : "binary"
            otaInfo.value = null
            if (fileKind.value === "binary") return
            otaInfo.value = await tauriApi.otaInfo(selected)
            otaManufacturer.value = compactHex(otaInfo.value.manufacturer)
            otaHwVersion.value = compactHex(otaInfo.value.hardwareVersion)
            otaSwVersion.value = compactHex(otaInfo.value.softwareVersion)
            if (otaInfo.value.deviceAddress !== null) otaDev.value = otaInfo.value.deviceAddress
        } catch (error) {
            otaError.value = errorMessage(error)
            otaFilePath.value = ""
            otaFileName.value = ""
            fileKind.value = "binary"
            otaInfo.value = null
        }
    }

    function updateElapsed() {
        otaElapsedSeconds.value = Math.floor((Date.now() - otaStartedAt) / 1000)
    }

    async function startOta() {
        otaError.value = ""
        otaResult.value = null
        if (session.statusOperationActive) {
            otaError.value = "车辆状态操作尚未完成，请稍后再开始 OTA"
            return
        }
        if (!session.supportsOta) {
            otaError.value = "当前通道不支持 OTA，请使用蓝牙或串口"
            return
        }
        if (otaActive.value) {
            otaError.value = "已有 OTA 更新正在进行"
            return
        }
        if (!otaFilePath.value) {
            otaError.value = "请先选择固件文件"
            return
        }
        otaActive.value = true
        otaCancelling.value = false
        otaProgress.value = 0
        otaElapsedSeconds.value = 0
        otaError.value = ""
        otaResult.value = null
        otaStartedAt = Date.now()
        clearInterval(otaClock)
        otaClock = setInterval(updateElapsed, 1000)
        try {
            const manufacturer = fixedHexBytes(4, "厂家编码").parse(otaManufacturer.value)
            const hardwareVersion = fixedHexBytes(2, "硬件版本").parse(otaHwVersion.value)
            const softwareVersion = fixedHexBytes(2, "软件版本").parse(otaSwVersion.value)
            if (!Number.isInteger(cycles.value) || cycles.value < 1 || cycles.value > 100) {
                throw new Error("更新次数必须是1到100之间的整数")
            }
            if (!await otaProgressReady) throw new Error("OTA 进度监听未就绪")
            const summary = await tauriApi.otaStart({
                path: otaFilePath.value,
                device: otaDev.value,
                manufacturer,
                hardwareVersion,
                softwareVersion,
                fileKind: fileKind.value,
                cycles: cycles.value,
            })
            otaProgress.value = 100
            otaResult.value = {
                type: "success",
                message: `更新完成：${summary.completedCycles}/${cycles.value} 次成功`,
            }
        } catch (error) {
            const message = errorMessage(error)
            if (isOtaCancelled(error)) {
                otaResult.value = {
                    type: "warn",
                    message: `更新已终止：当前进度 ${otaProgress.value}%`,
                }
            } else {
                otaError.value = message
                otaResult.value = { type: "error", message }
            }
            session.handleDisconnected(error)
        } finally {
            updateElapsed()
            clearInterval(otaClock)
            otaActive.value = false
            otaCancelling.value = false
        }
    }

    async function cancelOta() {
        if (!otaActive.value || otaCancelling.value) return
        otaCancelling.value = true
        otaError.value = ""
        try {
            await tauriApi.otaCancel()
        } catch (error) {
            otaError.value = errorMessage(error)
            otaCancelling.value = false
        }
    }

    const otaProgressReady = tauriApi
        .onOtaProgress((progress) => {
            if (!otaActive.value) return
            otaProgress.value = progress
        })
        .then(() => true)
        .catch((error) => {
            otaError.value = errorMessage(error, "OTA 进度监听启动失败")
            return false
        })

    watch(() => session.revision, () => {
        writing.value = false
        clearInterval(otaClock)
        otaActive.value = false
        otaCancelling.value = false
        otaError.value = ""
        otaResult.value = null
        clearResponses()
    })

    return {
        supportsOta, writing, commLog,
        readDev, readStart, readCount, readRegResp,
        writeDev, writeStart, writeValues, writeRegResp,
        readCoilDev, readCoilStart, readCoilCount, readCoilResp,
        writeCoilDev, writeCoilStart, writeCoilStates, writeCoilResp,
        otaFileName, otaDev, otaManufacturer, otaHwVersion, otaSwVersion, fileKind, otaInfo, cycles,
        otaActive, otaCancelling, otaProgress,
        otaElapsedSeconds, otaError, otaResult,
        sendReadRegisters, sendWriteRegisters, sendReadCoils, sendWriteCoils,
        clearLogs, selectFirmware, startOta, cancelOta,
    }
})
