import { z } from "zod"

const split = (value) => value.trim().split(/[\s,]+/).filter(Boolean)

export const hexBytes = (label = "HEX") => z.string()
    .transform((value) => value.replace(/\s+/g, ""))
    .pipe(z.string().min(2, `${label}不能为空`).regex(/^[0-9a-fA-F]+$/, `${label}包含非法字符`))
    .refine((value) => value.length % 2 === 0, `${label}长度必须为偶数`)
    .transform((value) => value.match(/../g).map((byte) => parseInt(byte, 16)))

export const fixedHexBytes = (length, label) => hexBytes(label)
    .refine((bytes) => bytes.length === length, `${label}必须为${length}字节`)

export const hexWords = z.string().transform(split)
    .refine((tokens) => tokens.length > 0, "请至少输入一个写入值")
    .refine((tokens) => tokens.every((token) => /^[0-9a-fA-F]{1,4}$/.test(token)), "写入值必须是 16 位 HEX")
    .transform((tokens) => tokens.map((token) => parseInt(token, 16)))

export const coilStates = z.string().transform(split)
    .refine((tokens) => tokens.length > 0, "请至少输入一个线圈状态")
    .refine((tokens) => tokens.every((token) => ["0", "1", "false", "true"].includes(token.toLowerCase())), "线圈状态只支持 0/1/true/false")
    .transform((tokens) => tokens.map((token) => ["1", "true"].includes(token.toLowerCase())))

export const errorCode = (error) => error?.code ?? error?.error?.code ?? ""
const errorText = (error) => {
    if (typeof error === "string") return error
    return String(error?.message ?? error?.error?.message ?? "")
}

export const isDisconnected = (error) => {
    if (["notConnected", "disconnected"].includes(errorCode(error))) return true
    return /(?:not\s*connected|disconnected|(?:BLE|串口|网络|设备|连接).*已断开|未连接设备|没有已连接的(?:设备|通道))/i
        .test(errorText(error))
}

export const isTimeout = (error) => {
    if (errorCode(error) === "timeout") return true
    return /(?:timed?\s*out|timeout|超时|no data received within)/i.test(errorText(error))
}
export const isOtaCancelled = (error) => {
    if (errorCode(error) === "otaCancelled") return true
    const message = typeof error === "string"
        ? error
        : error?.message ?? error?.error?.message ?? ""
    return typeof message === "string" && /OTA.*(停止|终止|取消|cancel)/i.test(message)
}

export function errorMessage(error, fallback = "操作失败，请重试") {
    if (error instanceof z.ZodError) return error.issues[0]?.message || "输入无效"
    const code = errorCode(error)
    const codeMessages = {
        busy: "设备正在执行其他操作",
        unsupported: "当前通道不支持此操作",
        notConnected: "未连接设备",
        notAuthenticated: "蓝牙尚未认证",
        otaCancelled: "OTA 更新已终止",
    }
    const value = typeof error === "string"
        ? error
        : error?.message ?? error?.error?.message ?? codeMessages[code] ?? error
    if (value == null) return fallback
    const message = typeof value === "object" ? JSON.stringify(value) : String(value ?? "")
    return message
        .replace(/^Error:\s*/i, "")
        .replace(/no data received within (\d+) ms/i, (_, ms) => `设备在 ${Number(ms) / 1000} 秒内没有返回数据`)
        .replace(/\s+/g, " ")
        .trim() || fallback
}
