import { onScopeDispose, reactive, watch } from "vue"
import { defineStore } from "pinia"
import { useSessionStore } from "./session"
import { tauriApi } from "../services/tauri"
import {
  errorMessage,
  isOtaCancelled,
  parseCoilStates,
  parseHexBytes,
  parseRegisterValues,
} from "../services/protocol"
import type {
  CommunicationLog,
  ModbusResponse,
  OtaFileInfo,
  OtaFileKind,
  OtaResultMessage,
  TransactionState,
} from "../types"

const emptyResponse = (): TransactionState => ({ request: "", raw: "", data: "", error: "" })
const spacedHex = (bytes: number[]) =>
  bytes.map((byte) => byte.toString(16).padStart(2, "0")).join(" ")
const compactHex = (bytes: number[]) =>
  bytes.map((byte) => byte.toString(16).padStart(2, "0").toUpperCase()).join("")

export const useTerminalStore = defineStore("terminal", () => {
  const session = useSessionStore()
  const protocol = reactive({
    writing: false,
    log: [] as CommunicationLog[],
    registers: { read: emptyResponse(), write: emptyResponse() },
    coils: { read: emptyResponse(), write: emptyResponse() },
  })
  const ota = reactive({
    path: "",
    fileName: "",
    device: 2,
    manufacturer: "00003132",
    deviceType: "0000",
    hardwareVersion: "",
    softwareVersion: "",
    description: "",
    fileKind: "binary" as OtaFileKind,
    cycles: 1,
    info: null as OtaFileInfo | null,
    active: false,
    cancelling: false,
    progress: 0,
    elapsedSeconds: 0,
    error: "",
    result: null as OtaResultMessage | null,
  })

  let nextLogId = 1
  let otaStartedAt = 0
  let otaClock: ReturnType<typeof setInterval> | undefined

  function addLog(label: string, response: TransactionState) {
    protocol.log.unshift({
      id: nextLogId++,
      time: new Date().toLocaleTimeString("zh-CN", { hour12: false }),
      type: response.error ? "error" : "success",
      label,
      ...response,
    })
    if (protocol.log.length > 50) protocol.log.length = 50
  }

  async function exchange(
    label: string,
    response: TransactionState,
    send: () => Promise<ModbusResponse>,
  ) {
    if (protocol.writing) return
    let blocked = ""
    if (session.runtime.statusActive) blocked = "车辆状态操作尚未完成，请稍后再试"
    else if (ota.active) blocked = "OTA 更新中，其他协议操作已暂停"
    if (blocked) {
      Object.assign(response, emptyResponse(), { error: blocked })
      addLog(label, response)
      return
    }

    protocol.writing = true
    Object.assign(response, emptyResponse())
    try {
      const result = await send()
      Object.assign(response, {
        request: spacedHex(result.request),
        raw: spacedHex(result.response),
        data: spacedHex(result.data),
        error: "",
      })
    } catch (error) {
      Object.assign(response, emptyResponse(), { error: errorMessage(error) })
      session.handleDisconnected(error)
    } finally {
      addLog(label, response)
      protocol.writing = false
    }
  }

  function readRegisters(request: { device: number; start: number; count: number }) {
    return exchange("读寄存器", protocol.registers.read, () => tauriApi.readRegisters(request))
  }

  function writeRegisters(request: { device: number; start: number; valuesText: string }) {
    return exchange("写寄存器", protocol.registers.write, () =>
      tauriApi.writeRegisters({
        device: request.device,
        start: request.start,
        values: parseRegisterValues(request.valuesText),
      }),
    )
  }

  function readCoils(request: { device: number; start: number; count: number }) {
    return exchange("读线圈", protocol.coils.read, () => tauriApi.readCoils(request))
  }

  function writeCoils(request: { device: number; start: number; valuesText: string }) {
    return exchange("写线圈", protocol.coils.write, () =>
      tauriApi.writeCoils({
        device: request.device,
        start: request.start,
        values: parseCoilStates(request.valuesText),
      }),
    )
  }

  function clearLog() {
    protocol.log.length = 0
    clearResponses()
  }

  function clearResponses() {
    Object.assign(protocol.registers.read, emptyResponse())
    Object.assign(protocol.registers.write, emptyResponse())
    Object.assign(protocol.coils.read, emptyResponse())
    Object.assign(protocol.coils.write, emptyResponse())
  }

  function firmwareFileName(path: string): string {
    let decoded = path
    for (let attempt = 0; attempt < 2; attempt += 1) {
      try {
        const next = decodeURIComponent(decoded)
        if (next === decoded) break
        decoded = next
      } catch {
        break
      }
    }
    return decoded.replace(/\\/g, "/").split("/").filter(Boolean).at(-1) || decoded
  }

  function firmwareExtension(path: string): string {
    const name = firmwareFileName(path).split(/[?#]/, 1)[0]
    const dot = name.lastIndexOf(".")
    return dot < 0 ? "" : name.slice(dot + 1).toLowerCase()
  }

  async function selectFirmware() {
    ota.error = ""
    ota.result = null
    try {
      const path = await tauriApi.pickFirmware()
      if (!path) return
      ota.path = path
      ota.fileName = firmwareFileName(path)
      ota.fileKind = firmwareExtension(path) === "ota" ? "packaged" : "binary"
      ota.info = ota.fileKind === "packaged" ? await tauriApi.otaInfo(path) : null
      ota.deviceType = ota.info ? compactHex(ota.info.deviceType) : "0000"
      ota.description = ota.info?.description ?? ""
      if (!ota.info) return

      ota.manufacturer = compactHex(ota.info.manufacturer)
      ota.hardwareVersion = compactHex(ota.info.hardwareVersion)
      ota.softwareVersion = compactHex(ota.info.softwareVersion)
      if (ota.info.deviceAddress !== null) ota.device = ota.info.deviceAddress
    } catch (error) {
      ota.error = errorMessage(error)
      ota.path = ""
      ota.fileName = ""
      ota.fileKind = "binary"
      ota.info = null
    }
  }

  async function startOta() {
    ota.error = ""
    ota.result = null
    if (session.runtime.statusActive) {
      ota.error = "车辆状态操作尚未完成，请稍后再开始 OTA"
      return
    }
    if (!session.supportsOta) {
      ota.error = "当前通道不支持 OTA，请使用蓝牙或串口"
      return
    }
    if (ota.active) {
      ota.error = "已有 OTA 更新正在进行"
      return
    }
    if (!ota.path) {
      ota.error = "请先选择固件文件"
      return
    }

    ota.active = true
    ota.cancelling = false
    ota.progress = 0
    ota.elapsedSeconds = 0
    otaStartedAt = Date.now()
    const updateElapsed = () => {
      ota.elapsedSeconds = Math.floor((Date.now() - otaStartedAt) / 1000)
    }
    clearInterval(otaClock)
    otaClock = setInterval(updateElapsed, 1000)

    try {
      if (!(await otaProgressReady)) throw new Error("OTA 进度监听未就绪")
      await tauriApi.otaStart({
        path: ota.path,
        isOta: ota.fileKind === "packaged",
        request: {
          manufacturer: parseHexBytes(ota.manufacturer, 4, "厂家编码"),
          deviceType: parseHexBytes(ota.deviceType, 2, "产品类型"),
          hardwareVersion: parseHexBytes(ota.hardwareVersion, 2, "硬件版本"),
          softwareVersion: parseHexBytes(ota.softwareVersion, 2, "软件版本"),
          deviceAddress: ota.device,
          description: ota.description,
        },
        cycles: ota.cycles,
      })
      ota.progress = 100
      ota.result = { type: "success", message: `更新完成：${ota.cycles} 次成功` }
    } catch (error) {
      const message = errorMessage(error)
      const cancelled = isOtaCancelled(error)
      ota.result = cancelled
        ? { type: "warn", message: `更新已终止：当前进度 ${ota.progress}%` }
        : { type: "error", message }
      if (!cancelled) ota.error = message
      session.handleDisconnected(error)
    } finally {
      updateElapsed()
      clearInterval(otaClock)
      ota.active = false
      ota.cancelling = false
    }
  }

  async function cancelOta() {
    if (!ota.active || ota.cancelling) return
    ota.cancelling = true
    ota.error = ""
    try {
      await tauriApi.otaCancel()
    } catch (error) {
      ota.error = errorMessage(error)
      ota.cancelling = false
    }
  }

  let removeOtaProgressListener: (() => void) | undefined
  const otaProgressReady = tauriApi
    .onOtaProgress((progress) => {
      if (ota.active) ota.progress = progress
    })
    .then((unlisten) => {
      removeOtaProgressListener = unlisten
      return true
    })
    .catch((error) => {
      ota.error = errorMessage(error, "OTA 进度监听启动失败")
      return false
    })
  onScopeDispose(() => {
    clearInterval(otaClock)
    removeOtaProgressListener?.()
  })

  watch(
    () => session.runtime.revision,
    () => {
      protocol.writing = false
      clearInterval(otaClock)
      ota.active = false
      ota.cancelling = false
      ota.error = ""
      ota.result = null
      clearResponses()
    },
  )

  return {
    protocol,
    ota,
    readRegisters,
    writeRegisters,
    readCoils,
    writeCoils,
    clearLog,
    selectFirmware,
    startOta,
    cancelOta,
  }
})
