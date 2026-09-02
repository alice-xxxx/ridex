import { invoke } from "@tauri-apps/api/core"
import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import { open, save } from "@tauri-apps/plugin-dialog"
import { platform } from "@tauri-apps/plugin-os"

import type {
  ConnectRequest,
  Connection,
  ConnectionClosedEvent,
  DiscoverRequest,
  DiscoveredEndpoint,
  LicenseStatus,
  ModbusRequest,
  ModbusResponse,
  NetworkCredentials,
  NetworkPlatform,
  OtaFileInfo,
  OtaPackRequest,
  OtaRequest,
  OtaStartResult,
  ReadModbusParam,
  WriteCoilsParam,
  WriteRegistersParam,
} from "../types"

type BackendDiscoveredDevice =
  | { ble: { name: string; address: string; rssi: number | null } }
  | { serial: { path: string; name: string } }

function connectionFrom(request: ConnectRequest): Connection {
  if (request.transport === "ble") {
    return {
      transport: "ble",
      endpoint: request.address,
      name: request.name,
      authenticated: false,
      supportsOta: true,
    }
  }
  if (request.transport === "serial") {
    return {
      transport: "serial",
      endpoint: request.path,
      name: request.path,
      authenticated: true,
      supportsOta: true,
    }
  }
  return {
    transport: "network",
    endpoint: request.name,
    name: request.name,
    authenticated: true,
    supportsOta: false,
  }
}

function discoveredEndpoint(device: BackendDiscoveredDevice): DiscoveredEndpoint {
  if ("ble" in device) {
    return {
      transport: "ble",
      id: device.ble.address,
      name: device.ble.name,
      signal: device.ble.rssi,
    }
  }
  return {
    transport: "serial",
    id: device.serial.path,
    name: device.serial.name,
    signal: null,
  }
}

const invokeModbus = (protocol: ModbusRequest): Promise<ModbusResponse> =>
  invoke("modbus_tx", { protocol, timeoutMs: 10000 })

const sourceFirmwareExtensions = [
  "bin",
  "hex",
  "ihex",
  "ihx",
  "tek",
  "tekhex",
  "srec",
  "s19",
  "s28",
  "s37",
  "mot",
  "srecord",
  "txt",
  "titxt",
  "ti-txt",
  "vmem",
  "mem",
  "elf",
  "axf",
  "out",
]

export const tauriApi = {
  supportsSerial: () => ["windows", "macos", "linux"].includes(platform()),
  licenseStatus: (): Promise<LicenseStatus | null> => invoke("license_status"),
  exitApp: (): Promise<void> => invoke("exit_app"),

  discover: (request: DiscoverRequest): Promise<DiscoveredEndpoint[]> =>
    invoke<BackendDiscoveredDevice[]>("discover", { request }).then((devices) =>
      devices.map(discoveredEndpoint),
    ),
  networkCredentials: (platform: NetworkPlatform): Promise<NetworkCredentials> =>
    invoke("network_credentials", { platform }),
  connect: async (request: ConnectRequest): Promise<Connection> => {
    await invoke("connect", { request })
    return connectionFrom(request)
  },
  disconnect: (): Promise<void> => invoke("disconnect"),
  authenticate: (vehicleCode: string): Promise<void> =>
    invoke("authenticate", { vehicleCode, timeoutMs: 5000 }),

  readRegisters: (param: ReadModbusParam): Promise<ModbusResponse> =>
    invokeModbus({ operation: "readRegisters", ...param }),
  writeRegisters: (param: WriteRegistersParam): Promise<ModbusResponse> =>
    invokeModbus({ operation: "writeRegisters", ...param }),
  readCoils: (param: ReadModbusParam): Promise<ModbusResponse> =>
    invokeModbus({ operation: "readCoils", ...param }),
  writeCoils: (param: WriteCoilsParam): Promise<ModbusResponse> =>
    invokeModbus({ operation: "writeCoils", ...param }),

  otaInfo: (path: string): Promise<OtaFileInfo> => invoke<OtaFileInfo>("ota_info", { path }),
  otaPack: (request: OtaPackRequest): Promise<void> => invoke<void>("ota_pack", { ...request }),
  otaStart: (request: OtaRequest): Promise<OtaStartResult> =>
    invoke<OtaStartResult>("ota_start", { ...request }),
  otaCancel: (): Promise<void> => invoke("ota_cancel"),
  onOtaProgress: (handler: (progress: number) => void): Promise<UnlistenFn> =>
    listen<number>("ota-progress", ({ payload }) => handler(payload)),
  onConnectionClosed: (handler: (event: ConnectionClosedEvent) => void): Promise<UnlistenFn> =>
    listen<ConnectionClosedEvent>("connection-closed", ({ payload }) => handler(payload)),

  pickFirmware: (): Promise<string | null> =>
    open({
      multiple: false,
      filters: [{ name: "固件文件", extensions: [...sourceFirmwareExtensions, "ota"] }],
    }),
  pickFirmwareSource: (): Promise<string | null> =>
    open({
      multiple: false,
      directory: false,
      filters: [{ name: "源固件文件", extensions: sourceFirmwareExtensions }],
    }),
  saveOta: (defaultPath: string): Promise<string | null> =>
    save({ defaultPath, filters: [{ name: "OTA 固件", extensions: ["ota"] }] }),
}
