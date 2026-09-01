import type {
  BitRange,
  ComponentCatalog,
  ContentDefinition,
  RegisterDefinition,
  StatusItem,
  StatusItemKind,
  StatusBlocks,
  StatusSnapshot,
} from "../types"

function findBlock(
  sourceCatalog: ComponentCatalog,
  kind: "coil" | "register",
  address: number,
): { start: number; count: number } | null {
  const blocks = kind === "coil" ? sourceCatalog.coilReadBlocks : sourceCatalog.readBlocks
  return (
    blocks.find(({ start, count }) => address >= start && address < start + count) ??
    null
  )
}

function cloneContent(content: ContentDefinition, value: unknown = null): ContentDefinition {
  return { ...content, value }
}

function unreadItem(
  address: number,
  definition: RegisterDefinition,
  kind: StatusItemKind,
): StatusItem {
  return {
    address,
    kind,
    loaded: false,
    bytes: definition.bytes,
    label: definition.label,
    writable: Boolean(definition.writable),
    rawHex: null,
    ...(definition.writePrefixHex ? { writePrefixHex: definition.writePrefixHex } : {}),
    content: definition.content.map((entry) => cloneContent(entry)),
  }
}

function expandBits(bits: BitRange[] | undefined, width: number): number[] {
  const ranges = bits ?? [[0, width - 1]]
  return ranges.flatMap(([start, end]) => {
    const low = Math.max(0, Math.min(Number(start), Number(end)))
    const high = Math.min(width - 1, Math.max(Number(start), Number(end)))
    return high < low ? [] : Array.from({ length: high - low + 1 }, (_, index) => low + index)
  })
}

function bytesToBigInt(bytes: number[]): bigint {
  return bytes.reduce((value, byte) => (value << 8n) | BigInt(byte), 0n)
}

function bigIntToNumber(value: bigint): number {
  if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new Error("数值超过 JavaScript 安全整数范围")
  }
  return Number(value)
}

function readBits(bytes: number[], bits?: BitRange[]): number {
  const word = bytesToBigInt(bytes)
  const value = expandBits(bits, bytes.length * 8).reduce(
    (result, bit, index) => result | (((word >> BigInt(bit)) & 1n) << BigInt(index)),
    0n,
  )
  return bigIntToNumber(value)
}

function writeBits(bytes: number[], bits: BitRange[] | undefined, value: number): number[] {
  let word = bytesToBigInt(bytes)
  const positions = expandBits(bits, bytes.length * 8)
  const nextValue = BigInt(value)

  positions.forEach((bit, index) => {
    const mask = 1n << BigInt(bit)
    if ((nextValue >> BigInt(index)) & 1n) word |= mask
    else word &= ~mask
  })

  return bytes.map((_byte, index) => {
    const shift = BigInt((bytes.length - index - 1) * 8)
    return Number((word >> shift) & 0xffn)
  })
}

function numberValue(bytes: number[]): number {
  return bigIntToNumber(bytesToBigInt(bytes))
}

function hexValue(bytes: number[]): string {
  return `0x${bytes
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("")
    .toUpperCase()}`
}

function bytesFromHex(value: unknown, expectedBytes: number): number[] {
  const clean = String(value).replace(/^0x/i, "")
  if (!new RegExp(`^[0-9a-fA-F]{${expectedBytes * 2}}$`).test(clean)) {
    throw new Error(`RAW 必须是 ${expectedBytes} 字节十六进制`)
  }
  return Array.from({ length: expectedBytes }, (_, index) =>
    Number.parseInt(clean.slice(index * 2, index * 2 + 2), 16),
  )
}

function bytesFromUnsigned(value: number, length: number): number[] {
  let remaining = BigInt(value)
  const bytes = Array(length).fill(0)
  for (let index = length - 1; index >= 0; index -= 1) {
    bytes[index] = Number(remaining & 0xffn)
    remaining >>= 8n
  }
  return bytes
}

function bytesFromSigned(value: number, length: number): number[] {
  let remaining = BigInt.asUintN(length * 8, BigInt(value))
  const bytes = Array(length).fill(0)
  for (let index = length - 1; index >= 0; index -= 1) {
    bytes[index] = Number(remaining & 0xffn)
    remaining >>= 8n
  }
  return bytes
}

function bytesFromUnsignedLittle(value: number, length: number): number[] {
  let remaining = BigInt(value)
  const bytes = Array(length).fill(0)
  for (let index = 0; index < length; index += 1) {
    bytes[index] = Number(remaining & 0xffn)
    remaining >>= 8n
  }
  return bytes
}

function pad(value: number): string {
  return String(value).padStart(2, "0")
}

function fromBcd(byte: number): number | null {
  const high = byte >> 4
  const low = byte & 0x0f
  return high <= 9 && low <= 9 ? high * 10 + low : null
}

function toBcd(value: number): number {
  return (Math.floor(value / 10) << 4) | value % 10
}

function validDateTime(
  year: number,
  month: number,
  day: number,
  hour = 0,
  minute = 0,
  second = 0,
): boolean {
  if (
    !Number.isInteger(year) ||
    year < 0 ||
    year > 9999 ||
    month < 1 ||
    month > 12 ||
    hour < 0 ||
    hour > 23 ||
    minute < 0 ||
    minute > 59 ||
    second < 0 ||
    second > 59
  )
    return false
  const leap = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0)
  const days = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
  return day >= 1 && day <= days[month - 1]
}

function parseDateTimeInput(
  input: unknown,
): [number, number, number, number, number, number] | null {
  const match = String(input).match(/^(\d{4})-(\d{2})-(\d{2})[T\s](\d{2}):(\d{2})(?::(\d{2}))?$/)
  if (!match) return null
  const values = match
    .slice(1)
    .map((value, index) => (index === 5 && value == null ? 0 : Number(value))) as [
    number,
    number,
    number,
    number,
    number,
    number,
  ]
  return validDateTime(...values) ? values : null
}

function parseDateInput(input: unknown): [number, number, number] | null {
  const match = String(input).match(/^(\d{4})-(\d{2})-(\d{2})$/)
  if (!match) return null
  const values = match.slice(1).map(Number) as [number, number, number]
  return validDateTime(...values) ? values : null
}

function scaledValue(raw: number, content: ContentDefinition): number {
  if (content.scale === undefined) return raw
  const value = raw * Number(content.scale)
  return content.precision === undefined ? value : Number(value.toFixed(content.precision))
}

function codecValue(content: ContentDefinition, bytes: number[]): unknown {
  switch (content.codec) {
    case "low_u8":
      return bytes.at(-1)
    case "speed_offset": {
      const raw = bytes.at(-1) ?? 0
      return (raw & 0x80 ? -1 : 1) * (raw & 0x7f)
    }
    case "ignition_high":
      return `标识=0x${bytes[0].toString(16).padStart(2, "0").toUpperCase()}, 次数=${bytes[1]}`
    case "controller_aux": {
      const raw = numberValue(bytes)
      const parts = []
      if ((raw >> 15) & 1) parts.push(`SOC ${(raw >> 8) & 0x7f}%`)
      parts.push(`额定电压 ${raw & 0xff}V`)
      return parts.join("、")
    }
    case "speed_stats":
      return `平均${bytes[0]}km/h, 最高${bytes[1]}km/h`
    case "run_state": {
      const mode = ["Boot", "APP"][bytes[1]] ?? `未知(${bytes[1]})`
      return bytes[0] === 0
        ? mode
        : `${mode}, 高字节=0x${bytes[0].toString(16).padStart(2, "0").toUpperCase()}`
    }
    case "signed_current": {
      const magnitude = readBits(bytes, content.magnitudeBits)
      const sign = readBits(bytes, content.signBits) ? -1 : 1
      const multiplier =
        Number(content.multiplierBase ?? 1) + readBits(bytes, content.multiplierBits)
      return magnitude * multiplier * sign
    }
    case "raw_voltage_5v": {
      const raw = readBits(bytes, content.bits)
      const voltage = (raw * Number(content.scale)).toFixed(content.precision ?? 2)
      return `${raw}（${voltage} V）`
    }
    case "hex_value":
      return hexValue(bytes)
    case "version_4byte":
      return bytes.map((byte) => byte.toString(16).padStart(2, "0").toUpperCase()).join(".")
    case "signal_4g": {
      const raw = numberValue(bytes)
      if (raw === 0) return "无信号，未联网"
      return raw <= 5 ? `信号强度 ${raw}` : `非法值(${raw})`
    }
    case "gps_signal": {
      const raw = numberValue(bytes)
      return raw === 0 ? "未定位" : `已定位，${raw}颗卫星`
    }
    case "signed_integer": {
      const raw = bytesToBigInt(bytes)
      const width = BigInt(bytes.length * 8)
      const signMask = 1n << (width - 1n)
      const value = Number(raw & signMask ? raw - (1n << width) : raw)
      return scaledValue(value, content)
    }
    case "signed_i8": {
      const raw = readBits(bytes, content.bits)
      return raw & 0x80 ? raw - 0x100 : raw
    }
    case "little_u16": {
      const raw = bytes.reduceRight((value, byte, index) => value | (byte << (index * 8)), 0)
      return scaledValue(raw, content)
    }
    case "little_u32": {
      const raw = bytes.reduceRight((value, byte, index) => value + byte * 2 ** (index * 8), 0)
      return scaledValue(raw, content)
    }
    case "little_f32": {
      const view = new DataView(Uint8Array.from(bytes).buffer)
      const raw = view.getFloat32(0, true)
      return content.precision === undefined ? raw : Number(raw.toFixed(content.precision))
    }
    case "bluetooth_status": {
      const high = bytes[0]
      const low = bytes[1]
      const connection =
        ["未连接", "已连接"][high] ??
        `非法(0x${high.toString(16).padStart(2, "0").toUpperCase()})`
      return low === 0
        ? connection
        : `${connection}, 低字节=0x${low.toString(16).padStart(2, "0").toUpperCase()}`
    }
    default:
      return undefined
  }
}

function parseContentValue(content: ContentDefinition, bytes: number[], rawHex: string): unknown {
  const special = codecValue(content, bytes)
  if (special !== undefined) return special

  switch (content.kind) {
    case "hex":
      return rawHex
    case "text": {
      const end = bytes.indexOf(0)
      return String.fromCharCode(...(end < 0 ? bytes : bytes.slice(0, end)))
    }
    case "switch":
      return Boolean(readBits(bytes, content.bits))
    case "select":
      return content.bits ? readBits(bytes, content.bits) : numberValue(bytes)
    case "value": {
      const raw = content.bits ? readBits(bytes, content.bits) : numberValue(bytes)
      return scaledValue(raw, content)
    }
    case "datetime": {
      const year = (bytes[0] << 8) | bytes[1]
      return `${String(year).padStart(4, "0")}-${pad(bytes[2])}-${pad(bytes[3])} ${pad(bytes[4])}:${pad(bytes[5])}:${pad(bytes[6])}`
    }
    case "date": {
      const century = fromBcd(bytes[0])
      const year = fromBcd(bytes[1])
      const month = fromBcd(bytes[2])
      const day = fromBcd(bytes[3])
      if (century === null || year === null || month === null || day === null) {
        return `无效BCD(${rawHex})`
      }
      return `${String(century * 100 + year).padStart(4, "0")}-${pad(month)}-${pad(day)}`
    }
    case "password": {
      const nibbles = bytes.flatMap((byte) => [(byte >> 4) & 0x0f, byte & 0x0f]).slice(0, 6)
      return !nibbles.every((value) => value === 0x0e)
    }
    default:
      return rawHex
  }
}

function parseRegisterBytes(
  address: number,
  bytes: number[],
  sourceCatalog: ComponentCatalog,
): StatusItem {
  const definition = sourceCatalog.registers[String(address)]
  if (!definition) throw new Error(`寄存器 R${address} 未定义`)
  if (bytes.length !== definition.bytes) {
    throw new Error(`寄存器 R${address} 需要 ${definition.bytes} 字节`)
  }

  const rawHex = hexValue(bytes)
  return {
    ...unreadItem(address, definition, "register"),
    loaded: true,
    rawHex,
    content: definition.content.map((content) =>
      cloneContent(content, parseContentValue(content, bytes, rawHex)),
    ),
  }
}

export function parseCoilValue(
  address: number,
  value: unknown,
  sourceCatalog: ComponentCatalog,
): StatusItem {
  const definition = sourceCatalog.coils[String(address)]
  if (!definition) throw new Error(`线圈 C${address} 未定义`)
  const active = Boolean(value)
  return {
    ...unreadItem(address, definition, "coil"),
    loaded: true,
    rawHex: active ? "0x01" : "0x00",
    content: definition.content.map((content) => cloneContent(content, active)),
  }
}

export function parseStatusData(
  registerBlocks: StatusBlocks,
  coilBlocks: StatusBlocks,
  sourceCatalog: ComponentCatalog,
): StatusSnapshot {
  const registerDefinitions = sourceCatalog.registers
  const coilDefinitions = sourceCatalog.coils
  const categoryDefinitions = sourceCatalog.categories
  const registers = Object.entries(registerDefinitions).map(([address, definition]) => {
    const numericAddress = Number(address)
    const block = findBlock(sourceCatalog, "register", numericAddress)
    const data = block ? (registerBlocks[block.start] ?? null) : null
    const offset = block ? (numericAddress - block.start) * 2 : -1
    return data && offset >= 0 && data.length >= offset + definition.bytes
      ? parseRegisterBytes(
          numericAddress,
          data.slice(offset, offset + definition.bytes),
          sourceCatalog,
        )
      : unreadItem(numericAddress, definition, "register")
  })

  const coils = Object.entries(coilDefinitions).map(([address, definition]) => {
    const numericAddress = Number(address)
    const block = findBlock(sourceCatalog, "coil", numericAddress)
    const data = block ? (coilBlocks[block.start] ?? null) : null
    const bitOffset = block ? numericAddress - block.start : -1
    return data && bitOffset >= 0 && data.length * 8 > bitOffset
      ? parseCoilValue(numericAddress, (data[bitOffset >> 3] >> (bitOffset & 7)) & 1, sourceCatalog)
      : unreadItem(numericAddress, definition, "coil")
  })

  const registerLookup = new Map(registers.map((item) => [item.address, item]))
  const coilLookup = new Map(coils.map((item) => [item.address, item]))
  const categories = Object.entries(categoryDefinitions).map(([id, definition]) => ({
    id,
    name: definition.name,
    registers: (definition.registers ?? [])
      .map((address) => registerLookup.get(address))
      .filter((item): item is StatusItem => item !== undefined),
    coils: (definition.coils ?? [])
      .map((address) => coilLookup.get(address))
      .filter((item): item is StatusItem => item !== undefined),
  }))

  return { categories, registers, coils }
}

function numericInput(content: ContentDefinition, input: unknown): number {
  if (content.codec === "hex_value" && /^0x[0-9a-f]+$/i.test(String(input))) {
    return Number.parseInt(String(input).slice(2), 16)
  }
  return Number(input)
}

function fieldWidth(content: ContentDefinition, item: StatusItem): number {
  const bits = content.writeBits ?? content.bits
  return bits ? expandBits(bits, item.bytes * 8).length : item.bytes * 8
}

function rawNumericInput(content: ContentDefinition, input: unknown): number {
  const value = numericInput(content, input)
  if (content.scale === undefined) return value
  const raw = value / Number(content.scale)
  return Math.round(raw)
}

export function validateRegisterInput(
  item: StatusItem,
  contentIndex: number,
  input: unknown,
): string | null {
  const content = item.content[contentIndex]
  if (item.kind !== "register") return "目标不是寄存器"
  if (!content) return "寄存器字段不存在"
  if (!item.loaded || !item.rawHex) return "寄存器尚未读取"
  if (!item.writable) return "寄存器不允许写入"

  if (content.kind === "hex") {
    const clean = String(input).replace(/^0x/i, "")
    return new RegExp(`^[0-9a-fA-F]{${item.bytes * 2}}$`).test(clean)
      ? null
      : `HEX 格式必须正好为 ${item.bytes} 字节`
  }

  if (content.kind === "text") {
    const text = String(input)
    if (text.length === 0) return "文本内容不能为空"
    if (!/^[\x20-\x7E]+$/.test(text)) return "文本内容仅支持可打印 ASCII 字符"
    if (text.length > item.bytes) return `文本内容不能超过 ${item.bytes} 个 ASCII 字符`
    return null
  }

  if (content.kind === "password") {
    return /^\d{1,6}$/.test(String(input)) ? null : "点火密码需为 1~6 位数字"
  }

  if (content.kind === "datetime") {
    return parseDateTimeInput(input) ? null : "日期时间格式无效"
  }

  if (content.kind === "date") {
    return parseDateInput(input) ? null : "日期格式无效"
  }

  if (content.kind === "switch") {
    return [true, false, 0, 1, "0", "1", "true", "false"].includes(
      input as boolean | number | string,
    )
      ? null
      : "开关值无效"
  }

  if (!new Set(["select", "value"]).has(content.kind)) {
    return `暂不支持修改 ${content.kind} 类型`
  }

  if (typeof input === "string" && input.trim() === "") return "数值不能为空"
  const value = numericInput(content, input)
  if (!Number.isFinite(value)) return "需为有效数字"
  if (content.min !== undefined && value < Number(content.min)) return `不能小于 ${content.min}`
  if (content.max !== undefined && value > Number(content.max)) return `不能大于 ${content.max}`

  if (
    content.kind === "select" &&
    content.options?.length &&
    !content.options.some((option) => Number(option.value) === value)
  ) {
    return "请选择有效选项"
  }

  if (content.codec === "speed_offset") {
    return Number.isInteger(value) ? null : "数值必须为整数"
  }

  const raw = content.scale === undefined ? value : value / Number(content.scale)
  const roundedRaw = Math.round(raw)
  if (!Number.isFinite(raw) || Math.abs(raw - roundedRaw) > 1e-8) {
    return content.scale === undefined ? "数值必须为整数" : `数值必须符合步进 ${content.scale}`
  }
  if (!Number.isSafeInteger(roundedRaw)) return "原始值超过安全整数范围"

  const width = fieldWidth(content, item)
  const signed = content.codec === "signed_integer"
  const minRaw = signed ? -(2 ** (width - 1)) : 0
  const maxRaw = signed ? 2 ** (width - 1) - 1 : 2 ** width - 1
  if (roundedRaw < minRaw || roundedRaw > maxRaw) {
    return `原始值需在 ${minRaw}~${maxRaw} 范围内`
  }
  return null
}

function switchInput(input: unknown): number {
  return input === true || input === 1 || input === "1" || input === "true" ? 1 : 0
}

function encodeText(input: unknown, length: number): number[] {
  const result: number[] = Array(length).fill(0)
  Array.from(String(input)).forEach((character, index) => {
    result[index] = character.charCodeAt(0)
  })
  return result
}

function encodePassword(input: unknown, currentBytes: number[]): number[] {
  const nibbles = String(input).split("").map(Number)
  while (nibbles.length < 6) nibbles.push(0x0e)
  const result = [...currentBytes]
  result[0] = (nibbles[0] << 4) | nibbles[1]
  result[1] = (nibbles[2] << 4) | nibbles[3]
  result[2] = (nibbles[4] << 4) | nibbles[5]
  return result
}

function encodeDateTime(input: unknown, currentBytes: number[]): number[] {
  const parsed = parseDateTimeInput(input)
  if (!parsed) throw new Error("日期时间格式无效")
  const [year, month, day, hour, minute, second] = parsed
  const result = [...currentBytes]
  result.splice(0, 7, year >> 8, year & 0xff, month, day, hour, minute, second)
  return result
}

function encodeDate(input: unknown, currentBytes: number[]): number[] {
  const parsed = parseDateInput(input)
  if (!parsed) throw new Error("日期格式无效")
  const [year, month, day] = parsed
  const result = [...currentBytes]
  result.splice(0, 4, toBcd(Math.floor(year / 100)), toBcd(year % 100), toBcd(month), toBcd(day))
  return result
}

function applyPrefix(bytes: number[], prefixHex?: string): number[] {
  if (!prefixHex) return bytes
  const clean = String(prefixHex).replace(/^0x/i, "")
  if (!/^(?:[0-9a-fA-F]{2})+$/.test(clean)) throw new Error("写入前缀格式无效")
  const prefix = Array.from({ length: clean.length / 2 }, (_, index) =>
    Number.parseInt(clean.slice(index * 2, index * 2 + 2), 16),
  )
  if (prefix.length > bytes.length) throw new Error("写入前缀长于寄存器数据")
  return [...prefix, ...bytes.slice(prefix.length)]
}

function bytesToWords(bytes: number[]): number[] {
  if (bytes.length % 2 !== 0) throw new Error("寄存器字节数必须为偶数")
  const values: number[] = []
  for (let index = 0; index < bytes.length; index += 2) {
    values.push((bytes[index] << 8) | bytes[index + 1])
  }
  return values
}

export function encodeRegisterWrite(
  item: StatusItem,
  contentIndex: number,
  input: unknown,
  sourceCatalog: ComponentCatalog,
): { values: number[]; expectedRawHex: string; optimisticItem: StatusItem } {
  const error = validateRegisterInput(item, contentIndex, input)
  if (error) throw new Error(error)

  const content = item.content[contentIndex]
  const currentBytes = bytesFromHex(item.rawHex, item.bytes)
  let nextBytes: number[]

  if (content.codec === "low_u8") {
    nextBytes = [...currentBytes]
    nextBytes[nextBytes.length - 1] = Number(input)
  } else if (content.codec === "speed_offset") {
    nextBytes = [...currentBytes]
    const value = Number(input)
    nextBytes[nextBytes.length - 1] = (value < 0 ? 0x80 : 0) | Math.abs(value)
  } else if (content.codec === "little_u16" || content.codec === "little_u32") {
    nextBytes = bytesFromUnsignedLittle(rawNumericInput(content, input), item.bytes)
  } else if (content.codec === "signed_integer") {
    nextBytes = content.bits
      ? writeBits(currentBytes, content.bits, rawNumericInput(content, input))
      : bytesFromSigned(rawNumericInput(content, input), item.bytes)
  } else if (content.kind === "hex") {
    nextBytes = bytesFromHex(input, item.bytes)
  } else if (content.kind === "text") {
    nextBytes = encodeText(input, item.bytes)
  } else if (content.kind === "password") {
    nextBytes = encodePassword(input, currentBytes)
  } else if (content.kind === "datetime") {
    nextBytes = encodeDateTime(input, currentBytes)
  } else if (content.kind === "date") {
    nextBytes = encodeDate(input, currentBytes)
  } else if (content.kind === "switch") {
    nextBytes = writeBits(currentBytes, content.bits, switchInput(input))
  } else if (content.kind === "select" || content.kind === "value") {
    const raw = rawNumericInput(content, input)
    nextBytes = content.bits
      ? writeBits(currentBytes, content.bits, raw)
      : bytesFromUnsigned(raw, item.bytes)
  } else {
    throw new Error(`暂不支持修改 ${content.kind} 类型`)
  }

  let expectedBytes = nextBytes
  if (content.verifyValue !== undefined) {
    expectedBytes = content.bits
      ? writeBits(currentBytes, content.bits, content.verifyValue)
      : bytesFromUnsigned(content.verifyValue, item.bytes)
  }
  let writeBytes = nextBytes
  if (content.writeBits) {
    if (content.kind === "switch") {
      writeBytes = writeBits(currentBytes, content.writeBits, switchInput(input))
    } else if (content.kind === "select" || content.kind === "value") {
      writeBytes = writeBits(currentBytes, content.writeBits, rawNumericInput(content, input))
    }
  }
  writeBytes = applyPrefix(writeBytes, content.writePrefixHex ?? item.writePrefixHex)
  const expectedRawHex = hexValue(expectedBytes)
  return {
    values: bytesToWords(writeBytes),
    expectedRawHex,
    optimisticItem: parseRegisterBytes(item.address, expectedBytes, sourceCatalog),
  }
}

export function formatContentValue(
  content: ContentDefinition,
  item: StatusItem,
): string {
  if (!item.loaded || content.value === null || content.value === undefined) return "—"

  if (content.kind === "password") return content.value ? "已设置" : "未设置"
  if (content.kind === "switch") {
    return String((content.value ? content.on : content.off) ?? content.value)
  }
  if (content.kind === "select") {
    const option = content.options?.find((entry) => Number(entry.value) === Number(content.value))
    return option ? String(option.label) : String(content.value)
  }

  let value = content.value
  if (content.scale !== undefined && content.precision !== undefined && typeof value === "number") {
    value = value.toFixed(content.precision)
  }
  return content.unit === undefined || content.unit === ""
    ? String(value)
    : `${value} ${content.unit}`
}

export function runtimeItemKey(
  item: Pick<StatusItem, "kind" | "address">,
): string {
  return `${item.kind}:${item.address}`
}
