import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { open, save } from "@tauri-apps/plugin-dialog"
import { z } from "zod"

// 基础字段校验。
const byte = z.number().int().min(0).max(255)
const word = z.number().int().min(0).max(65535)

// 前端请求校验。
const discoverRequestSchema = z.discriminatedUnion("transport", [
    z.object({
        transport: z.literal("ble"),
        timeoutMs: z.number().int().min(500).max(30000),
        serviceUuid: z.string().nullable(),
    }).strict(),
    z.object({ transport: z.literal("serial") }).strict(),
])
const connectRequestSchema = z.discriminatedUnion("transport", [
    z.object({
        transport: z.literal("ble"),
        address: z.string().min(1),
        name: z.string(),
        timeoutMs: z.number().int().positive(),
    }).strict(),
    z.object({
        transport: z.literal("serial"),
        path: z.string().min(1),
        baudRate: z.number().int().min(300).max(3000000),
    }).strict(),
    z.object({
        transport: z.literal("network"),
        name: z.string().length(16),
        platform: z.enum(["production", "test"]),
        username: z.string(),
        password: z.string(),
        timeoutMs: z.number().int().positive(),
    }).strict(),
])
const modbusRequestSchema = z.discriminatedUnion("operation", [
    z.object({
        operation: z.literal("readRegisters"),
        device: byte,
        start: word,
        count: z.number().int().min(1).max(125),
    }).strict(),
    z.object({
        operation: z.literal("writeRegisters"),
        device: byte,
        start: word,
        values: z.array(word).min(1).max(123),
    }).strict(),
    z.object({
        operation: z.literal("readCoils"),
        device: byte,
        start: word,
        count: z.number().int().min(1).max(2000),
    }).strict(),
    z.object({
        operation: z.literal("writeCoils"),
        device: byte,
        start: word,
        values: z.array(z.boolean()).min(1).max(1968),
    }).strict(),
])
const otaInfoRequestSchema = z.object({ path: z.string().min(1) }).strict()
const licenseStatusSchema = z.object({
    authorized: z.boolean(),
    ready: z.boolean(),
    deviceId: z.string(),
    message: z.string().nullable(),
}).strict()
const otaPackRequestSchema = z.object({
    inputPath: z.string().min(1),
    outputPath: z.string().min(1),
    manufacturer: z.array(byte).length(4),
    deviceType: z.array(byte).length(2),
    hardwareVersion: z.array(byte).length(2),
    softwareVersion: z.array(byte).length(2),
    deviceAddress: byte,
    description: z.string().max(20),
}).strict()
// Tauri 命令参数校验，结构必须和 Rust command 的参数一致。
const commandRequestSchema = (schema) => z.object({ request: schema }).strict()
const discoverCommandRequestSchema = commandRequestSchema(discoverRequestSchema)
const connectCommandRequestSchema = commandRequestSchema(connectRequestSchema)
const otaInfoCommandRequestSchema = commandRequestSchema(otaInfoRequestSchema)
const networkCredentialsRequestSchema = z.object({
    platform: z.enum(["production", "test"]),
}).strict()
const otaRequestSchema = z.object({
    path: z.string().min(1),
    device: byte,
    manufacturer: z.array(byte).length(4),
    hardwareVersion: z.array(byte).length(2),
    softwareVersion: z.array(byte).length(2),
    fileKind: z.enum(["binary", "packaged"]),
    cycles: z.number().int().min(1).max(100),
}).strict()

const authenticateRequestSchema = z.object({
    vehicleCode: z.string(),
    timeoutMs: z.number().int().positive(),
}).strict()

const modbusCommandRequestSchema = z.object({
    protocol: modbusRequestSchema,
    timeoutMs: z.number().int().positive(),
}).strict()

// 后端返回值校验。
const channelCapsSchema = z.boolean()
const networkCredentialsSchema = z.object({
    saved: z.boolean(),
    username: z.string().nullable(),
}).strict()
const discoveredDevicesSchema = z.array(z.union([
    z.object({
        ble: z.object({
            name: z.string(),
            address: z.string(),
            rssi: z.number().int().nullable(),
        }).strict(),
    }).strict(),
    z.object({
        serial: z.object({
            path: z.string(),
            name: z.string(),
        }).strict(),
    }).strict(),
]))

const txSchema = z.object({
    request: z.array(byte),
    response: z.array(byte),
    data: z.array(byte),
})
const otaFileInfoSchema = z.object({
    fileCrc: z.number().int().nonnegative(),
    fileSize: z.number().int().nonnegative(),
    manufacturer: z.array(byte).length(4),
    deviceType: z.array(byte).length(2),
    hardwareVersion: z.array(byte).length(2),
    softwareVersion: z.array(byte).length(2),
    deviceAddress: byte.nullable(),
    description: z.string(),
    upgradeCrc: z.number().int().nonnegative(),
    upgradeSize: z.number().int().nonnegative(),
})

const otaProgressSchema = z.number().int().min(0).max(100)
const unitSchema = z.null()
const byteArraySchema = z.preprocess(
    (value) => value instanceof Uint8Array ? Array.from(value) : value,
    z.array(byte),
)
const otaStartResultSchema = z.object({
    bytesPerCycle: z.number().int().nonnegative(),
}).strict()
const otaSummarySchema = z.object({
    completedCycles: z.number().int().nonnegative(),
    bytesPerCycle: z.number().int().nonnegative(),
    elapsedMs: z.number().int().nonnegative(),
})
const connectionClosedSchema = z.object({
    transport: z.enum(["ble", "serial", "network"]),
    reason: z.string(),
})

// 统一执行 Tauri command：校验请求、调用后端、校验返回值。
const command = async (name, responseSchema, requestSchema, args) => {
    const input = requestSchema ? requestSchema.parse(args) : undefined
    const result = await invoke(name, input)
    return responseSchema ? responseSchema.parse(result) : result
}

// 后端数据转换为前端使用的统一结构。
const connectionFromRequest = (request) => {
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
        supportsOta: true,
    }
}

const normalizeDiscoveredDevice = (device) => {
    if (device.ble) {
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

const toOtaSummary = (request, result) => otaSummarySchema.parse({
    completedCycles: request.cycles,
    bytesPerCycle: result.bytesPerCycle,
    elapsedMs: 0,
})

// Tauri command API。
const channelCaps = () => command("channel_caps", channelCapsSchema)
const licenseStatus = () => command("license_status", licenseStatusSchema)
const exitApp = () => command("exit_app", unitSchema)

const discover = async (request) => {
    const devices = await command(
        "discover",
        discoveredDevicesSchema,
        discoverCommandRequestSchema,
        { request },
    )
    return devices.map(normalizeDiscoveredDevice)
}

const networkCredentials = (platform) => command(
    "network_credentials",
    networkCredentialsSchema,
    networkCredentialsRequestSchema,
    { platform },
)

const connect = async (request) => {
    await command(
        "connect",
        unitSchema,
        connectCommandRequestSchema,
        { request },
    )
    return connectionFromRequest(request)
}

const disconnect = () => command("disconnect", unitSchema)

const channelAuth = (vehicleCode) => command(
    "authenticate",
    unitSchema,
    authenticateRequestSchema,
    { vehicleCode, timeoutMs: 5000 },
)

const modbusTx = (request, timeoutMs = 10000) => command(
    "modbus_tx",
    txSchema,
    modbusCommandRequestSchema,
    { protocol: request, timeoutMs },
)

const otaInfo = (path) => command(
    "ota_info",
    otaFileInfoSchema,
    otaInfoCommandRequestSchema,
    { request: { path } },
)

const otaPack = (request) => command(
    "ota_pack",
    otaFileInfoSchema,
    otaPackRequestSchema,
    request,
)

const otaStart = async (request) => {
    const result = await command(
        "ota_start",
        otaStartResultSchema,
        otaRequestSchema,
        request,
    )
    return toOtaSummary(request, result)
}

const readFile = (path) => command(
    "plugin:fs|read_file",
    byteArraySchema,
    otaInfoRequestSchema,
    { path },
)

const otaCancel = () => command("ota_cancel", unitSchema)

// Tauri 事件和文件选择器 API。
const onOtaProgress = (handler) => listen("ota-progress", ({ payload }) => {
    handler(otaProgressSchema.parse(payload))
})

const onConnectionClosed = (handler) => listen("connection-closed", ({ payload }) => {
    handler(connectionClosedSchema.parse(payload))
})

const pickFirmware = () => open({
    multiple: false,
    filters: [{ name: "固件文件", extensions: ["bin", "ota"] }],
})

const pickBin = () => open({
    multiple: false,
    directory: false,
    filters: [{ name: "BIN 固件", extensions: ["bin"] }],
})

const saveOta = (defaultPath) => save({
    defaultPath,
    filters: [{ name: "OTA 固件", extensions: ["ota"] }],
})

export const tauriApi = {
    channelCaps,
    licenseStatus,
    exitApp,
    discover,
    networkCredentials,
    connect,
    disconnect,
    channelAuth,
    modbusTx,
    otaInfo,
    otaPack,
    otaStart,
    readFile,
    otaCancel,
    onOtaProgress,
    onConnectionClosed,
    pickFirmware,
    pickBin,
    saveOta,
}
