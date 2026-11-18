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
) {
  const blocks = kind === "coil" ? sourceCatalog.coilReadBlocks : sourceCatalog.readBlocks
  return blocks.find(({ start, count }) => address >= start && address < start + count)
}

function createItem(
  address: number,
  definition: RegisterDefinition,
  kind: StatusItemKind,
  bytes?: number[],
): StatusItem {
  const rawHex = bytes ? hexValue(bytes) : null
  return {
    address,
    kind,
    loaded: Boolean(bytes),
    bytes: definition.bytes,
    label: definition.label,
    writable: Boolean(definition.writable),
    rawHex,
    ...(definition.writePrefixHex ? { writePrefixHex: definition.writePrefixHex } : {}),
    content: definition.content.map((content) => ({
      ...content,
      value: !bytes ? null : kind === "coil"
        ? Boolean(bytes[0])
        : parseContentValue(content, bytes, rawHex!),
    })),
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
  if (!bits) return numberValue(bytes)
  const word = bytesToBigInt(bytes)
  const value = expandBits(bits, bytes.length * 8).reduce(
    (result, bit, index) => result | (((word >> BigInt(bit)) & 1n) << BigInt(index)),
    0n,
  )
  return bigIntToNumber(value)
}

function writeBits(bytes: number[], bits: BitRange[] | undefined, value: number): number[] {
  if (!bits) return numberBytes(value, bytes.length)
  let word = bytesToBigInt(bytes)
  const positions = expandBits(bits, bytes.length * 8)
  const nextValue = BigInt(value)

  positions.forEach((bit, index) => {
    const mask = 1n << BigInt(bit)
    if ((nextValue >> BigInt(index)) & 1n) word |= mask
    else word &= ~mask
  })

  return numberBytes(word, bytes.length)
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

function numberBytes(value: number | bigint, length: number, little = false): number[] {
  const word = BigInt(value)
  return Array.from({ length }, (_, index) => {
    const shift = BigInt((little ? index : length - index - 1) * 8)
    return Number((word >> shift) & 0xffn)
  })
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
  const values = match.slice(1).map((value) => Number(value ?? 0)) as [
    number, number, number, number, number, number,
  ]
  return validDateTime(...values) ? values : null
}

function parseDateInput(input: unknown): [number, number, number] | null {
  const match = String(input).match(/^(\d{4})-(\d{2})-(\d{2})$/)
  if (!match) return null
  const values = match.slice(1).map(Number) as [number, number, number]
  return validDateTime(...values) ? values : null
}

export function parseUnit(value = "") {
  const text = value.trim()
  const match = text.match(/^([+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?)(.*)$/i)
  const scale = match ? Number(match[1]) : 1
  if (!Number.isFinite(scale) || scale === 0) throw new Error("单位比例必须是有限非零数")
  return { scale, unit: match ? match[2].trim() : text }
}

function scaledValue(raw: number, content: ContentDefinition): number {
  return raw * parseUnit(content.unit).scale
}

function parseContentValue(content: ContentDefinition, bytes: number[], rawHex: string): unknown {
  if (content.codec === "speed_offset" || content.codec === "signed_current") {
    const raw = content.codec === "speed_offset" ? (bytes.at(-1) ?? 0) : bytes[6]
    const multiplier = content.codec === "signed_current" && (bytes[12] & 0x40) ? 2 : 1
    return scaledValue((raw & 0x80 ? -1 : 1) * (raw & 0x7f) * multiplier, content)
  }

  switch (content.kind) {
    case "version":
      return bytes.join(".")
    case "hex":
      return rawHex
    case "string": {
      const end = bytes.indexOf(0)
      return String.fromCharCode(...(end < 0 ? bytes : bytes.slice(0, end)))
    }
    case "switch":
      return Boolean(readBits(bytes, content.bits))
    case "select":
      return readBits(bytes, content.bits)
    case "uint_le":
      return scaledValue(numberValue([...bytes].reverse()), content)
    case "float_le": {
      const view = new DataView(Uint8Array.from(bytes).buffer)
      return scaledValue(bytes.length === 8 ? view.getFloat64(0, true) : view.getFloat32(0, true), content)
    }
    case "int": {
      const width = content.bits ? expandBits(content.bits, bytes.length * 8).length : bytes.length * 8
      const raw = content.bits ? BigInt(readBits(bytes, content.bits)) : bytesToBigInt(bytes)
      return scaledValue(Number(BigInt.asIntN(width, raw)), content)
    }
    case "uint":
      return scaledValue(readBits(bytes, content.bits), content)
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

  return createItem(address, definition, "register", bytes)
}

export function parseCoilValue(
  address: number,
  value: unknown,
  sourceCatalog: ComponentCatalog,
): StatusItem {
  const definition = sourceCatalog.coils[String(address)]
  if (!definition) throw new Error(`线圈 C${address} 未定义`)
  return createItem(address, definition, "coil", [Number(Boolean(value))])
}

export function parseStatusData(
  registerBlocks: StatusBlocks,
  coilBlocks: StatusBlocks,
  sourceCatalog: ComponentCatalog,
): StatusSnapshot {
  const registers = Object.entries(sourceCatalog.registers).map(([address, definition]) => {
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
      : createItem(numericAddress, definition, "register")
  })

  const coils = Object.entries(sourceCatalog.coils).map(([address, definition]) => {
    const numericAddress = Number(address)
    const block = findBlock(sourceCatalog, "coil", numericAddress)
    const data = block ? (coilBlocks[block.start] ?? null) : null
    const bitOffset = block ? numericAddress - block.start : -1
    return data && bitOffset >= 0 && data.length * 8 > bitOffset
      ? parseCoilValue(numericAddress, (data[bitOffset >> 3] >> (bitOffset & 7)) & 1, sourceCatalog)
      : createItem(numericAddress, definition, "coil")
  })

  const registerLookup = new Map(registers.map((item) => [item.address, item]))
  const coilLookup = new Map(coils.map((item) => [item.address, item]))
  const categories = Object.entries(sourceCatalog.categories).map(([id, definition]) => ({
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

function fieldWidth(content: ContentDefinition, item: StatusItem): number {
  const bits = content.writeBits ?? content.bits
  return bits ? expandBits(bits, item.bytes * 8).length : item.bytes * 8
}

function rawNumericInput(content: ContentDefinition, input: unknown): number {
  return Math.round(Number(input) / parseUnit(content.unit).scale)
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

  if (content.kind === "string") {
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

  if (!["select", "uint", "int", "uint_le"].includes(content.kind)) {
    return `暂不支持修改 ${content.kind} 类型`
  }

  if (typeof input === "string" && input.trim() === "") return "数值不能为空"
  const value = Number(input)
  if (!Number.isFinite(value)) return "需为有效数字"
  if (content.minmax) {
    const [min, max] = content.minmax
    if (value < min) return `不能小于 ${min}`
    if (value > max) return `不能大于 ${max}`
  }

  if (
    content.kind === "select" &&
    content.options?.length &&
    !content.options.some((option) => Number(option.value) === value)
  ) {
    return "请选择有效选项"
  }

  const { scale } = parseUnit(content.unit)
  const raw = value / scale
  const roundedRaw = Math.round(raw)
  if (!Number.isFinite(raw) || Math.abs(raw - roundedRaw) > 1e-8) {
    return scale === 1 ? "数值必须为整数" : `数值必须符合步进 ${Math.abs(scale)}`
  }
  if (!Number.isSafeInteger(roundedRaw)) return "原始值超过安全整数范围"
  if (content.codec === "speed_offset") {
    return Math.abs(roundedRaw) <= 0x7f ? null : "原始值需在 -127~127 范围内"
  }

  const width = fieldWidth(content, item)
  const signed = content.kind === "int"
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

function encodeContent(content: ContentDefinition, input: unknown, bytes: number[]): number[] {
  if (content.codec === "speed_offset") {
    const result = [...bytes]
    const value = rawNumericInput(content, input)
    result[result.length - 1] = (value < 0 ? 0x80 : 0) | Math.abs(value)
    return result
  }

  switch (content.kind) {
    case "uint_le":
      return numberBytes(rawNumericInput(content, input), bytes.length, true)
    case "select":
    case "uint":
    case "int":
      return writeBits(bytes, content.bits, rawNumericInput(content, input))
    case "switch":
      return writeBits(bytes, content.bits, switchInput(input))
    case "hex":
      return bytesFromHex(input, bytes.length)
    case "string": {
      const text = String(input)
      return Array.from({ length: bytes.length }, (_, index) => text.charCodeAt(index) || 0)
    }
    case "password": {
      const nibbles = String(input).split("").map(Number)
      while (nibbles.length < 6) nibbles.push(0x0e)
      const password = [0, 2, 4].map((index) => (nibbles[index] << 4) | nibbles[index + 1])
      return [...password, ...bytes.slice(3)]
    }
    case "datetime": {
      const [year, ...rest] = parseDateTimeInput(input)!
      return [year >> 8, year & 0xff, ...rest, ...bytes.slice(7)]
    }
    case "date": {
      const [year, month, day] = parseDateInput(input)!
      return [toBcd(Math.floor(year / 100)), toBcd(year % 100), toBcd(month), toBcd(day), ...bytes.slice(4)]
    }
    default:
      throw new Error(`暂不支持修改 ${content.kind} 类型`)
  }
}

function applyPrefix(bytes: number[], prefixHex?: string): number[] {
  if (!prefixHex) return bytes
  const clean = String(prefixHex).replace(/^0x/i, "")
  if (!/^(?:[0-9a-fA-F]{2})+$/.test(clean)) throw new Error("写入前缀格式无效")
  const prefix = bytesFromHex(clean, clean.length / 2)
  if (prefix.length > bytes.length) throw new Error("写入前缀长于寄存器数据")
  return [...prefix, ...bytes.slice(prefix.length)]
}

function bytesToWords(bytes: number[]): number[] {
  if (bytes.length % 2 !== 0) throw new Error("寄存器字节数必须为偶数")
  return Array.from({ length: bytes.length / 2 }, (_, index) =>
    (bytes[index * 2] << 8) | bytes[index * 2 + 1],
  )
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
  const nextBytes = encodeContent(content, input, currentBytes)
  const expectedBytes = content.verifyValue === undefined ? nextBytes
    : writeBits(currentBytes, content.bits, content.verifyValue)
  let writeBytes = nextBytes
  if (content.writeBits && ["switch", "select", "uint", "int", "uint_le"].includes(content.kind)) {
    const value = content.kind === "switch" ? switchInput(input) : rawNumericInput(content, input)
    writeBytes = writeBits(currentBytes, content.writeBits, value)
  }
  const expectedRawHex = hexValue(expectedBytes)
  return {
    values: bytesToWords(applyPrefix(writeBytes, content.writePrefixHex ?? item.writePrefixHex)),
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

  const { unit } = parseUnit(content.unit)
  const value = typeof content.value === "number"
    ? String(Number(content.value.toFixed(6)))
    : String(content.value)
  return unit ? `${value} ${unit}` : value
}

export function runtimeItemKey(
  item: Pick<StatusItem, "kind" | "address">,
): string {
  return `${item.kind}:${item.address}`
}
