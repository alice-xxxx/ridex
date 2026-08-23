import { catalog as defaultCatalog } from "./catalog.js"

function catalogValue(sourceCatalog, key) {
    return sourceCatalog?.[key] ?? defaultCatalog[key]
}

function findBlock(sourceCatalog, kind, address) {
    const numericAddress = Number(address)
    const blocks = catalogValue(sourceCatalog, kind === "coil" ? "coilReadBlocks" : "readBlocks")
    return blocks.find(({ start, count }) => (
        numericAddress >= start && numericAddress < start + count
    )) ?? null
}

function cloneContent(content, value = null) {
    return { ...structuredClone(content), value }
}

function unreadItem(address, definition, kind) {
    return {
        address: Number(address),
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

function byteArray(value) {
    if (value == null) return null
    const source = value?.data ?? value
    if (!Array.isArray(source) && !ArrayBuffer.isView(source)) return null
    return Array.from(source, (byte) => Number(byte) & 0xFF)
}

function blockData(blocks, start) {
    if (!blocks) return null
    if (blocks instanceof Map) return byteArray(blocks.get(start) ?? blocks.get(String(start)))
    if (Array.isArray(blocks)) {
        const record = blocks.find((entry) => entry && typeof entry === "object"
            && !ArrayBuffer.isView(entry) && Number(entry.start) === start)
        return byteArray(record)
    }
    return byteArray(blocks[start] ?? blocks[String(start)])
}

function expandBits(bits, width) {
    const ranges = bits ?? [[0, width - 1]]
    return ranges.flatMap(([start, end]) => {
        const low = Math.max(0, Math.min(Number(start), Number(end)))
        const high = Math.min(width - 1, Math.max(Number(start), Number(end)))
        return high < low
            ? []
            : Array.from({ length: high - low + 1 }, (_, index) => low + index)
    })
}

function bytesToBigInt(bytes) {
    return bytes.reduce((value, byte) => (value << 8n) | BigInt(byte), 0n)
}

function bigIntToNumber(value) {
    if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
        throw new Error("数值超过 JavaScript 安全整数范围")
    }
    return Number(value)
}

function readBits(bytes, bits) {
    const word = bytesToBigInt(bytes)
    const value = expandBits(bits, bytes.length * 8).reduce(
        (result, bit, index) => result | (((word >> BigInt(bit)) & 1n) << BigInt(index)),
        0n,
    )
    return bigIntToNumber(value)
}

function writeBits(bytes, bits, value) {
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
        return Number((word >> shift) & 0xFFn)
    })
}

function numberValue(bytes) {
    return bigIntToNumber(bytesToBigInt(bytes))
}

function hexValue(bytes) {
    return `0x${bytes.map((byte) => byte.toString(16).padStart(2, "0")).join("").toUpperCase()}`
}

function bytesFromHex(value, expectedBytes) {
    const clean = String(value).replace(/^0x/i, "")
    if (!new RegExp(`^[0-9a-fA-F]{${expectedBytes * 2}}$`).test(clean)) {
        throw new Error(`RAW 必须是 ${expectedBytes} 字节十六进制`)
    }
    return Array.from(
        { length: expectedBytes },
        (_, index) => Number.parseInt(clean.slice(index * 2, index * 2 + 2), 16),
    )
}

function bytesFromUnsigned(value, length) {
    let remaining = BigInt(value)
    const bytes = Array(length).fill(0)
    for (let index = length - 1; index >= 0; index -= 1) {
        bytes[index] = Number(remaining & 0xFFn)
        remaining >>= 8n
    }
    return bytes
}

function bytesFromUnsignedLittle(value, length) {
    let remaining = BigInt(value)
    const bytes = Array(length).fill(0)
    for (let index = 0; index < length; index += 1) {
        bytes[index] = Number(remaining & 0xFFn)
        remaining >>= 8n
    }
    return bytes
}

function pad(value) {
    return String(value).padStart(2, "0")
}

function fromBcd(byte) {
    const high = byte >> 4
    const low = byte & 0x0F
    return high <= 9 && low <= 9 ? high * 10 + low : null
}

function toBcd(value) {
    return (Math.floor(value / 10) << 4) | (value % 10)
}

function validDateTime(year, month, day, hour = 0, minute = 0, second = 0) {
    if (!Number.isInteger(year) || year < 0 || year > 9999
        || month < 1 || month > 12 || hour < 0 || hour > 23
        || minute < 0 || minute > 59 || second < 0 || second > 59) return false
    const leap = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0)
    const days = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    return day >= 1 && day <= days[month - 1]
}

function parseDateTimeInput(input) {
    const match = String(input).match(
        /^(\d{4})-(\d{2})-(\d{2})[T\s](\d{2}):(\d{2})(?::(\d{2}))?$/,
    )
    if (!match) return null
    const values = match.slice(1).map((value, index) => index === 5 && value == null ? 0 : Number(value))
    return validDateTime(...values) ? values : null
}

function parseDateInput(input) {
    const match = String(input).match(/^(\d{4})-(\d{2})-(\d{2})$/)
    if (!match) return null
    const values = match.slice(1).map(Number)
    return validDateTime(...values) ? values : null
}

function scaledValue(raw, content) {
    if (content.scale === undefined) return raw
    const value = raw * Number(content.scale)
    return content.precision === undefined ? value : Number(value.toFixed(content.precision))
}

function codecValue(content, bytes) {
    switch (content.codec) {
        case "low_u8":
            return bytes.at(-1)
        case "speed_offset": {
            const raw = bytes.at(-1)
            return (raw & 0x80 ? -1 : 1) * (raw & 0x7F)
        }
        case "ignition_high":
            return `标识=0x${bytes[0].toString(16).padStart(2, "0").toUpperCase()}, 次数=${bytes[1]}`
        case "controller_aux": {
            const raw = numberValue(bytes)
            const parts = []
            if ((raw >> 15) & 1) parts.push(`SOC ${(raw >> 8) & 0x7F}%`)
            parts.push(`额定电压 ${raw & 0xFF}V`)
            return parts.join("、")
        }
        case "speed_stats":
            return `平均${bytes[0]}km/h, 最高${bytes[1]}km/h`
        case "run_state": {
            const mode = bytes[1] === 0 ? "Boot" : bytes[1] === 1 ? "APP" : `未知(${bytes[1]})`
            return bytes[0] === 0
                ? mode
                : `${mode}, 高字节=0x${bytes[0].toString(16).padStart(2, "0").toUpperCase()}`
        }
        case "signed_current": {
            const magnitude = readBits(bytes, content.magnitudeBits)
            const sign = readBits(bytes, content.signBits) ? -1 : 1
            const multiplier = Number(content.multiplierBase ?? 1) + readBits(bytes, content.multiplierBits)
            return magnitude * multiplier * sign
        }
        case "raw_voltage_5v": {
            const raw = readBits(bytes, content.bits)
            const voltage = (raw * Number(content.scale)).toFixed(content.precision ?? 2)
            return `${raw}（${voltage} V）`
        }
        case "hex_value":
            return hexValue(bytes)
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
        case "little_u16": {
            const raw = bytes.reduceRight((value, byte, index) => value | (byte << (index * 8)), 0)
            return scaledValue(raw, content)
        }
        case "little_u32": {
            const raw = bytes.reduceRight((value, byte, index) => value + byte * (2 ** (index * 8)), 0)
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
            const connection = high === 0
                ? "未连接"
                : high === 1 ? "已连接" : `非法(0x${high.toString(16).padStart(2, "0").toUpperCase()})`
            return low === 0
                ? connection
                : `${connection}, 低字节=0x${low.toString(16).padStart(2, "0").toUpperCase()}`
        }
        default:
            return undefined
    }
}

function parseContentValue(content, bytes, rawHex) {
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
            const parts = bytes.slice(0, 4).map(fromBcd)
            return parts.some((value) => value === null)
                ? `无效BCD(${rawHex})`
                : `${String(parts[0] * 100 + parts[1]).padStart(4, "0")}-${pad(parts[2])}-${pad(parts[3])}`
        }
        case "password": {
            const nibbles = bytes.flatMap((byte) => [(byte >> 4) & 0x0F, byte & 0x0F]).slice(0, 6)
            return !nibbles.every((value) => value === 0x0E)
        }
        default:
            return rawHex
    }
}

export function parseRegisterBytes(address, bytes, sourceCatalog = defaultCatalog) {
    const numericAddress = Number(address)
    const registerDefinitions = catalogValue(sourceCatalog, "registers")
    const definition = registerDefinitions[String(numericAddress)]
    if (!definition) throw new Error(`寄存器 R${numericAddress} 未定义`)
    const source = byteArray(bytes)
    if (!source || source.length !== definition.bytes) {
        throw new Error(`寄存器 R${numericAddress} 需要 ${definition.bytes} 字节`)
    }

    const rawHex = hexValue(source)
    return {
        ...unreadItem(numericAddress, definition, "register"),
        loaded: true,
        rawHex,
        content: definition.content.map((content) => (
            cloneContent(content, parseContentValue(content, source, rawHex))
        )),
    }
}

export function parseCoilValue(address, value, sourceCatalog = defaultCatalog) {
    const numericAddress = Number(address)
    const coilDefinitions = catalogValue(sourceCatalog, "coils")
    const definition = coilDefinitions[String(numericAddress)]
    if (!definition) throw new Error(`线圈 C${numericAddress} 未定义`)
    const active = Boolean(value)
    return {
        ...unreadItem(numericAddress, definition, "coil"),
        loaded: true,
        rawHex: active ? "0x01" : "0x00",
        content: definition.content.map((content) => cloneContent(content, active)),
    }
}

export function parseStatusData(registerBlocks = {}, coilBlocks = {}, sourceCatalog = defaultCatalog) {
    const registerDefinitions = catalogValue(sourceCatalog, "registers")
    const coilDefinitions = catalogValue(sourceCatalog, "coils")
    const categoryDefinitions = catalogValue(sourceCatalog, "categories")
    const registers = Object.entries(registerDefinitions).map(([address, definition]) => {
        const numericAddress = Number(address)
        const block = findBlock(sourceCatalog, "register", numericAddress)
        const data = block ? blockData(registerBlocks, block.start) : null
        const offset = block ? (numericAddress - block.start) * 2 : -1
        return data && offset >= 0 && data.length >= offset + definition.bytes
            ? parseRegisterBytes(numericAddress, data.slice(offset, offset + definition.bytes), sourceCatalog)
            : unreadItem(numericAddress, definition, "register")
    })

    const coils = Object.entries(coilDefinitions).map(([address, definition]) => {
        const numericAddress = Number(address)
        const block = findBlock(sourceCatalog, "coil", numericAddress)
        const data = block ? blockData(coilBlocks, block.start) : null
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
        registers: (definition.registers ?? []).map((address) => registerLookup.get(address)).filter(Boolean),
        coils: (definition.coils ?? []).map((address) => coilLookup.get(address)).filter(Boolean),
    }))

    return { categories, registers, coils }
}

function numericInput(content, input) {
    if (content.codec === "hex_value" && /^0x[0-9a-f]+$/i.test(String(input))) {
        return Number.parseInt(String(input).slice(2), 16)
    }
    return Number(input)
}

function fieldWidth(content, item) {
    return content.bits
        ? expandBits(content.bits, item.bytes * 8).length
        : item.bytes * 8
}

function rawNumericInput(content, item, input) {
    const value = numericInput(content, input)
    if (content.scale === undefined) return value
    const raw = value / Number(content.scale)
    return Math.round(raw)
}

export function validateRegisterInput(item, contentIndex, input) {
    const content = item?.content?.[contentIndex]
    if (!item || item.kind !== "register") return "目标不是寄存器"
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
        return [true, false, 0, 1, "0", "1", "true", "false"].includes(input)
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

    if (content.kind === "select" && content.options?.length
        && !content.options.some((option) => Number(option.value) === value)) {
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
    const maxRaw = 2 ** width - 1
    if (roundedRaw < 0 || roundedRaw > maxRaw) return `原始值需在 0~${maxRaw} 范围内`
    return null
}

function switchInput(input) {
    return input === true || input === 1 || input === "1" || input === "true" ? 1 : 0
}

function encodeText(input, length) {
    const result = Array(length).fill(0)
    Array.from(String(input)).forEach((character, index) => {
        result[index] = character.charCodeAt(0)
    })
    return result
}

function encodePassword(input, currentBytes) {
    const nibbles = String(input).split("").map(Number)
    while (nibbles.length < 6) nibbles.push(0x0E)
    const result = [...currentBytes]
    result[0] = (nibbles[0] << 4) | nibbles[1]
    result[1] = (nibbles[2] << 4) | nibbles[3]
    result[2] = (nibbles[4] << 4) | nibbles[5]
    return result
}

function encodeDateTime(input, currentBytes) {
    const [year, month, day, hour, minute, second] = parseDateTimeInput(input)
    const result = [...currentBytes]
    result.splice(0, 7, year >> 8, year & 0xFF, month, day, hour, minute, second)
    return result
}

function encodeDate(input, currentBytes) {
    const [year, month, day] = parseDateInput(input)
    const result = [...currentBytes]
    result.splice(0, 4, toBcd(Math.floor(year / 100)), toBcd(year % 100), toBcd(month), toBcd(day))
    return result
}

function applyPrefix(bytes, prefixHex) {
    if (!prefixHex) return bytes
    const clean = String(prefixHex).replace(/^0x/i, "")
    if (!/^(?:[0-9a-fA-F]{2})+$/.test(clean)) throw new Error("写入前缀格式无效")
    const prefix = Array.from(
        { length: clean.length / 2 },
        (_, index) => Number.parseInt(clean.slice(index * 2, index * 2 + 2), 16),
    )
    if (prefix.length > bytes.length) throw new Error("写入前缀长于寄存器数据")
    return [...prefix, ...bytes.slice(prefix.length)]
}

function bytesToWords(bytes) {
    if (bytes.length % 2 !== 0) throw new Error("寄存器字节数必须为偶数")
    const values = []
    for (let index = 0; index < bytes.length; index += 2) {
        values.push((bytes[index] << 8) | bytes[index + 1])
    }
    return values
}

export function encodeRegisterWrite(item, contentIndex, input, sourceCatalog = defaultCatalog) {
    const error = validateRegisterInput(item, contentIndex, input)
    if (error) throw new Error(error)

    const content = item.content[contentIndex]
    const currentBytes = bytesFromHex(item.rawHex, item.bytes)
    let nextBytes

    if (content.codec === "low_u8") {
        nextBytes = [...currentBytes]
        nextBytes[nextBytes.length - 1] = Number(input)
    } else if (content.codec === "speed_offset") {
        nextBytes = [...currentBytes]
        const value = Number(input)
        nextBytes[nextBytes.length - 1] = (value < 0 ? 0x80 : 0) | Math.abs(value)
    } else if (content.codec === "little_u16" || content.codec === "little_u32") {
        nextBytes = bytesFromUnsignedLittle(rawNumericInput(content, item, input), item.bytes)
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
        const raw = rawNumericInput(content, item, input)
        nextBytes = content.bits
            ? writeBits(currentBytes, content.bits, raw)
            : bytesFromUnsigned(raw, item.bytes)
    } else {
        throw new Error(`暂不支持修改 ${content.kind} 类型`)
    }

    nextBytes = applyPrefix(nextBytes, item.writePrefixHex)
    const expectedRawHex = hexValue(nextBytes)
    return {
        values: bytesToWords(nextBytes),
        expectedRawHex,
        optimisticItem: parseRegisterBytes(item.address, nextBytes, sourceCatalog),
    }
}

export function formatContentValue(content, item) {
    if (!item?.loaded || content?.value === null || content?.value === undefined) return "—"

    if (content.kind === "password") return content.value ? "已设置" : "未设置"
    if (content.kind === "switch") {
        const key = content.value ? "on" : "off"
        return Object.prototype.hasOwnProperty.call(content, key) ? String(content[key]) : String(content.value)
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

export function runtimeItemKey(item) {
    return `${item?.kind ?? "unknown"}:${Number(item?.address)}`
}
