interface ErrorPayload {
  code?: string
  message?: unknown
  error?: ErrorPayload
}

const ERROR_MESSAGES: Record<string, string> = {
  busy: "设备正在执行其他操作",
  unsupported: "当前通道不支持此操作",
  notConnected: "未连接设备",
  notAuthenticated: "蓝牙尚未认证",
  otaCancelled: "OTA 更新已终止",
}

function errorDetails(error: unknown): { code: string; message: unknown } {
  if (typeof error === "string") return { code: "", message: error }
  const value = error && typeof error === "object" ? (error as ErrorPayload) : {}
  return {
    code: value.code ?? value.error?.code ?? "",
    message: value.message ?? value.error?.message,
  }
}

export function parseHexBytes(value: string, length: number, label: string): number[] {
  const hex = value.replace(/\s+/g, "")
  if (!hex) throw new Error(`${label}不能为空`)
  if (!/^[0-9a-fA-F]+$/.test(hex)) throw new Error(`${label}包含非法字符`)
  if (hex.length % 2 !== 0) throw new Error(`${label}长度必须为偶数`)

  const bytes = hex.match(/../g)?.map((byte) => Number.parseInt(byte, 16)) ?? []
  if (bytes.length !== length) throw new Error(`${label}必须为${length}字节`)
  return bytes
}

export function parseRegisterValues(value: string): number[] {
  const tokens = value.trim().split(/[\s,]+/).filter(Boolean)
  if (!tokens.length) throw new Error("请至少输入一个写入值")
  if (!tokens.every((token) => /^(?:0x)?[0-9a-fA-F]+$/.test(token))) {
    throw new Error("写入值必须是十进制或十六进制")
  }

  const values = tokens.map((token) =>
    token.toLowerCase().startsWith("0x") || /[a-f]/i.test(token)
      ? Number.parseInt(token, 16)
      : Number.parseInt(token, 10),
  )
  if (values.some((number) => number < 0 || number > 0xffff)) {
    throw new Error("写入值必须在 0 到 65535 之间")
  }
  return values
}

export function parseCoilStates(value: string): boolean[] {
  const tokens = value.trim().split(/[\s,]+/).filter(Boolean)
  if (!tokens.length) throw new Error("请至少输入一个线圈状态")
  if (!tokens.every((token) => token === "0" || token === "1")) {
    throw new Error("线圈状态只支持 0/1")
  }
  return tokens.map((token) => token === "1")
}

export function isDisconnected(error: unknown): boolean {
  const { code, message } = errorDetails(error)
  if (["notConnected", "disconnected"].includes(code)) return true
  return /(?:not\s*connected|disconnected|(?:BLE|串口|网络|设备|连接).*已断开|未连接设备|没有已连接的(?:设备|通道))/i.test(
    String(message ?? ""),
  )
}

export function isTimeout(error: unknown): boolean {
  const { code, message } = errorDetails(error)
  return (
    code === "timeout" ||
    /(?:timed?\s*out|timeout|超时|no data received within)/i.test(String(message ?? ""))
  )
}

export function isOtaCancelled(error: unknown): boolean {
  const { code, message } = errorDetails(error)
  return code === "otaCancelled" || /OTA.*(停止|终止|取消|cancel)/i.test(String(message ?? ""))
}

export function errorMessage(error: unknown, fallback = "操作失败，请重试"): string {
  const details = errorDetails(error)
  const message = details.message ?? ERROR_MESSAGES[details.code] ?? error
  if (message == null) return fallback

  return (typeof message === "object" ? JSON.stringify(message) : String(message))
    .replace(/^Error:\s*/i, "")
    .replace(
      /no data received within (\d+) ms/i,
      (_, milliseconds) => `设备在 ${Number(milliseconds) / 1000} 秒内没有返回数据`,
    )
    .replace(/\s+/g, " ")
    .trim() || fallback
}
