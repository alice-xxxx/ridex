import { computed, ref, shallowRef, watch } from "vue"
import { defineStore } from "pinia"
import { componentCatalog } from "../registers/components.js"
import {
    encodeRegisterWrite,
    parseCoilValue,
    parseStatusData,
    runtimeItemKey,
} from "../registers/codec.js"
import { errorMessage, isDisconnected, isTimeout } from "../services/protocol.js"
import { tauriApi } from "../services/tauri.js"
import { useSessionStore } from "./session.js"
import { useTerminalStore } from "./terminal.js"

const AUTO_REFRESH_MS = 5000
const VERIFY_TIMEOUT_MS = 5000
const VERIFY_INTERVAL_MS = 250

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
const emptyBlocks = () => Object.create(null)

function sameHex(left, right) {
    return typeof left === "string"
        && typeof right === "string"
        && left.toUpperCase() === right.toUpperCase()
}

function hasCompleteRawHex(item) {
    return item?.loaded && Number.isInteger(item.bytes) && item.bytes > 0
        && new RegExp(`^0x[0-9a-fA-F]{${item.bytes * 2}}$`).test(item.rawHex || "")
}

function blockLabel(kind, block) {
    const prefix = kind === "coil" ? "C" : "R"
    const end = block.start + block.count - 1
    return `${prefix}${String(block.start).padStart(3, "0")}-${prefix}${String(end).padStart(3, "0")}`
}

function blockData(blocks, start) {
    const value = blocks[start] ?? blocks[String(start)]
    return value == null ? null : Array.from(value, (byte) => Number(byte) & 0xFF)
}

export const useComponentStatusStore = defineStore("component-status", () => {
    const session = useSessionStore()
    const terminal = useTerminalStore()

    const catalog = computed(() => componentCatalog(session.component))
    const snapshot = shallowRef({ categories: [], registers: [], coils: [] })
    const pendingWrites = shallowRef(new Map())
    const refreshing = ref(false)
    const progress = ref({ done: 0, total: 0 })
    const error = ref("")
    const writeError = ref("")
    const writingKey = ref(null)
    const autoRefresh = ref(true)
    const lastRefreshAt = ref(null)

    let registerBlocks = emptyBlocks()
    let coilBlocks = emptyBlocks()
    let autoTimer = null
    let refreshGeneration = 0
    let writeGeneration = 0

    const data = computed(() => {
        if (!pendingWrites.value.size) return snapshot.value
        const replace = (item) => pendingWrites.value.get(runtimeItemKey(item)) || item
        const registers = snapshot.value.registers.map(replace)
        const coils = snapshot.value.coils.map(replace)
        const currentItems = new Map(
            [...registers, ...coils].map((item) => [runtimeItemKey(item), item]),
        )
        const categories = snapshot.value.categories.map((category) => ({
            ...category,
            registers: category.registers.map((item) => currentItems.get(runtimeItemKey(item)) || item),
            coils: category.coils.map((item) => currentItems.get(runtimeItemKey(item)) || item),
        }))
        return { categories, registers, coils }
    })

    const inCooldown = computed(() => writingKey.value !== null
        || pendingWrites.value.size > 0 || terminal.writing || terminal.otaActive)

    function clearAutoTimer() {
        if (autoTimer !== null) clearTimeout(autoTimer)
        autoTimer = null
    }

    function canRefresh(revision = session.revision, componentId = session.component) {
        return revision === session.revision
            && componentId === session.component
            && session.authenticated
            && session.page === "status"
            && componentCatalog(componentId) !== null
            && !terminal.otaActive
    }

    function reset() {
        clearAutoTimer()
        refreshGeneration += 1
        writeGeneration += 1
        registerBlocks = emptyBlocks()
        coilBlocks = emptyBlocks()
        snapshot.value = catalog.value
            ? parseStatusData(registerBlocks, coilBlocks, catalog.value)
            : { categories: [], registers: [], coils: [] }
        pendingWrites.value = new Map()
        refreshing.value = false
        progress.value = { done: 0, total: 0 }
        error.value = ""
        writeError.value = ""
        writingKey.value = null
        autoRefresh.value = true
        lastRefreshAt.value = null
    }

    function scheduleAutoRefresh() {
        clearAutoTimer()
        if (!autoRefresh.value || !canRefresh()) return
        autoTimer = setTimeout(() => {
            autoTimer = null
            void refresh().finally(() => scheduleAutoRefresh())
        }, AUTO_REFRESH_MS)
    }

    function setAutoRefresh(enabled) {
        autoRefresh.value = Boolean(enabled)
        if (!autoRefresh.value) clearAutoTimer()
        else scheduleAutoRefresh()
    }

    function stopAutoRefresh() {
        autoRefresh.value = false
        clearAutoTimer()
    }

    async function requestBlock(kind, block, sourceCatalog, timeoutMs = 10000) {
        const coil = kind === "coil"
        const result = await tauriApi.modbusTx({
            operation: coil ? "readCoils" : "readRegisters",
            device: sourceCatalog.deviceAddress,
            start: block.start,
            count: block.count,
        }, timeoutMs)
        const received = result?.data
        if (!received || typeof received.length !== "number") {
            throw new Error(`${blockLabel(kind, block)} 响应不含数据`)
        }
        const expectedLength = coil ? Math.ceil(block.count / 8) : block.count * 2
        if ((!coil && received.length !== expectedLength) || (coil && received.length < expectedLength)) {
            throw new Error(`${blockLabel(kind, block)} 响应长度错误：期望${expectedLength}字节，收到${received.length}字节`)
        }
        return Array.from(received).slice(0, expectedLength)
    }

    function handleTransportError(cause) {
        if (isTimeout(cause)) stopAutoRefresh()
        session.handleDisconnected(cause)
    }

    async function refresh() {
        if (!canRefresh() || refreshing.value || writingKey.value !== null || terminal.writing) return false
        clearAutoTimer()
        const revision = session.revision
        const componentId = session.component
        const sourceCatalog = componentCatalog(componentId)
        const generation = ++refreshGeneration
        const blocks = [
            ...(sourceCatalog.readBlocks ?? []).map((block) => ["register", block]),
            ...(sourceCatalog.coilReadBlocks ?? []).map((block) => ["coil", block]),
        ]
        const nextRegisterBlocks = { ...registerBlocks }
        const nextCoilBlocks = { ...coilBlocks }
        const failures = []
        let completedBlocks = 0
        let stoppedByTimeout = false

        refreshing.value = true
        session.statusOperationActive = true
        error.value = ""
        writeError.value = ""
        progress.value = { done: 0, total: blocks.length }

        try {
            for (const [kind, block] of blocks) {
                if (!canRefresh(revision, componentId) || generation !== refreshGeneration) return false
                try {
                    const bytes = await requestBlock(kind, block, sourceCatalog)
                    if (!canRefresh(revision, componentId) || generation !== refreshGeneration) return false
                    if (kind === "coil") nextCoilBlocks[block.start] = bytes
                    else nextRegisterBlocks[block.start] = bytes
                    completedBlocks += 1
                } catch (cause) {
                    if (!canRefresh(revision, componentId) || generation !== refreshGeneration) return false
                    handleTransportError(cause)
                    if (isDisconnected(cause)) return false
                    failures.push(`${blockLabel(kind, block)}：${errorMessage(cause)}`)
                    if (isTimeout(cause)) {
                        stoppedByTimeout = true
                        break
                    }
                } finally {
                    if (generation === refreshGeneration) progress.value = {
                        done: progress.value.done + 1,
                        total: blocks.length,
                    }
                }
            }

            if (!canRefresh(revision, componentId) || generation !== refreshGeneration) return false
            registerBlocks = nextRegisterBlocks
            coilBlocks = nextCoilBlocks
            snapshot.value = parseStatusData(registerBlocks, coilBlocks, sourceCatalog)
            if (completedBlocks > 0) lastRefreshAt.value = new Date()
            if (failures.length) {
                const prefix = stoppedByTimeout ? "读取超时，自动刷新已停止" : "部分数据读取失败"
                error.value = `${prefix}\n${failures.map((message) => `• ${message}`).join("\n")}`
            }
            return failures.length === 0
        } catch (cause) {
            handleTransportError(cause)
            if (generation === refreshGeneration
                && revision === session.revision
                && componentId === session.component) error.value = errorMessage(cause)
            return false
        } finally {
            if (generation === refreshGeneration) {
                refreshing.value = false
                session.statusOperationActive = false
                scheduleAutoRefresh()
            }
        }
    }

    function findRegisterBlock(address, sourceCatalog = catalog.value) {
        const numericAddress = Number(address)
        return sourceCatalog?.readBlocks?.find(({ start, count }) => (
            numericAddress >= start && numericAddress < start + count
        )) ?? null
    }

    function findCoilBlock(address, sourceCatalog = catalog.value) {
        const numericAddress = Number(address)
        return sourceCatalog?.coilReadBlocks?.find(({ start, count }) => (
            numericAddress >= start && numericAddress < start + count
        )) ?? null
    }

    function currentRegister(address) {
        return snapshot.value.registers.find((item) => item.address === Number(address)) ?? null
    }

    function currentCoil(address) {
        return snapshot.value.coils.find((item) => item.address === Number(address)) ?? null
    }

    function updateVerifiedBlock(kind, block, bytes, sourceCatalog) {
        if (kind === "coil") coilBlocks = { ...coilBlocks, [block.start]: bytes }
        else registerBlocks = { ...registerBlocks, [block.start]: bytes }
        snapshot.value = parseStatusData(registerBlocks, coilBlocks, sourceCatalog)
    }

    async function verify(kind, address, expectedRawHex, revision, generation, componentId, sourceCatalog) {
        const block = kind === "coil"
            ? findCoilBlock(address, sourceCatalog)
            : findRegisterBlock(address, sourceCatalog)
        if (!block) throw new Error(`${kind === "coil" ? "线圈" : "寄存器"} ${address} 不在读取范围内`)
        const deadline = Date.now() + VERIFY_TIMEOUT_MS
        let lastReadError = null
        do {
            if (revision !== session.revision
                || componentId !== session.component
                || generation !== writeGeneration) return false
            try {
                const bytes = await requestBlock(kind, block, sourceCatalog, Math.max(1, deadline - Date.now()))
                if (revision !== session.revision
                    || componentId !== session.component
                    || generation !== writeGeneration) return false
                updateVerifiedBlock(kind, block, bytes, sourceCatalog)
                const actual = kind === "coil" ? currentCoil(address) : currentRegister(address)
                if (actual?.loaded && sameHex(actual.rawHex, expectedRawHex)) return true
                lastReadError = null
            } catch (cause) {
                if (isDisconnected(cause)) throw cause
                handleTransportError(cause)
                lastReadError = cause
            }
            if (Date.now() < deadline) await sleep(Math.min(VERIFY_INTERVAL_MS, deadline - Date.now()))
        } while (Date.now() < deadline)
        if (lastReadError) throw lastReadError
        return false
    }

    function writeBlockedMessage() {
        if (!session.authenticated || session.page !== "status") return "当前没有可写入的部件状态"
        if (terminal.otaActive) return "OTA 更新中，状态写入已暂停"
        if (writingKey.value !== null || pendingWrites.value.size) return "已有状态正在写入"
        if (terminal.writing) return "已有协议操作正在执行"
        return ""
    }

    function setPending(item) {
        pendingWrites.value = new Map(pendingWrites.value).set(runtimeItemKey(item), item)
    }

    function clearPending(key) {
        const next = new Map(pendingWrites.value)
        next.delete(key)
        pendingWrites.value = next
    }

    async function writeRegister(address, contentIndex, value) {
        const blocked = writeBlockedMessage()
        if (blocked) { writeError.value = blocked; return false }
        const item = currentRegister(address)
        if (!item || !hasCompleteRawHex(item)) {
            writeError.value = `寄存器 R${address} 尚未完整读取，不能写入`
            return false
        }
        const componentId = session.component
        const sourceCatalog = componentCatalog(componentId)
        let encoded
        try {
            encoded = encodeRegisterWrite(item, contentIndex, value, sourceCatalog)
        } catch (cause) {
            writeError.value = errorMessage(cause, "写入值无效")
            return false
        }
        const key = runtimeItemKey(item)
        const revision = session.revision
        const generation = ++writeGeneration
        setPending(encoded.optimisticItem)
        writingKey.value = key
        session.statusOperationActive = true
        writeError.value = ""
        error.value = ""
        clearAutoTimer()
        try {
            await tauriApi.modbusTx({
                operation: "writeRegisters",
                device: sourceCatalog.deviceAddress,
                start: item.address,
                values: encoded.values,
            })
            if (!await verify("register", item.address, encoded.expectedRawHex,
                revision, generation, componentId, sourceCatalog)) {
                throw new Error(`写入后未确认 R${item.address} 的目标值`)
            }
            return true
        } catch (cause) {
            handleTransportError(cause)
            if (revision === session.revision
                && componentId === session.component
                && generation === writeGeneration) writeError.value = errorMessage(cause)
            return false
        } finally {
            if (generation === writeGeneration) {
                clearPending(key)
                writingKey.value = null
                session.statusOperationActive = false
                scheduleAutoRefresh()
            }
        }
    }

    async function writeCoil(address, value) {
        const blocked = writeBlockedMessage()
        if (blocked) { writeError.value = blocked; return false }
        const item = currentCoil(address)
        if (!item || !hasCompleteRawHex(item)) {
            writeError.value = `线圈 C${address} 尚未完整读取，不能写入`
            return false
        }
        if (![true, false, 0, 1].includes(value)) {
            writeError.value = "线圈写入值必须是布尔值"
            return false
        }
        const active = Boolean(value)
        const componentId = session.component
        const sourceCatalog = componentCatalog(componentId)
        const optimisticItem = parseCoilValue(item.address, active, sourceCatalog)
        const key = runtimeItemKey(item)
        const revision = session.revision
        const generation = ++writeGeneration
        setPending(optimisticItem)
        writingKey.value = key
        session.statusOperationActive = true
        writeError.value = ""
        error.value = ""
        clearAutoTimer()
        try {
            await tauriApi.modbusTx({
                operation: "writeCoils",
                device: sourceCatalog.deviceAddress,
                start: item.address,
                values: [active],
            })
            if (!await verify("coil", item.address, optimisticItem.rawHex,
                revision, generation, componentId, sourceCatalog)) {
                throw new Error(`写入后未确认 C${item.address} 的目标值`)
            }
            return true
        } catch (cause) {
            handleTransportError(cause)
            if (revision === session.revision
                && componentId === session.component
                && generation === writeGeneration) writeError.value = errorMessage(cause)
            return false
        } finally {
            if (generation === writeGeneration) {
                clearPending(key)
                writingKey.value = null
                session.statusOperationActive = false
                scheduleAutoRefresh()
            }
        }
    }

    watch([() => session.revision, () => session.component], reset, { immediate: true, flush: "sync" })
    watch(
        [() => session.authenticated, () => session.page, () => session.component, () => terminal.otaActive, () => terminal.writing],
        ([authenticated, page, _componentId, otaActive, protocolBusy]) => {
            clearAutoTimer()
            if (!authenticated || page !== "status" || otaActive || protocolBusy) return
            const hasLoadedItem = snapshot.value.registers.some((item) => item.loaded)
                || snapshot.value.coils.some((item) => item.loaded)
            if (!hasLoadedItem && autoRefresh.value) void refresh()
            else scheduleAutoRefresh()
        },
        { immediate: true },
    )

    return {
        data,
        refreshing,
        progress,
        error,
        writeError,
        writingKey,
        autoRefresh,
        lastRefreshAt,
        inCooldown,
        refresh,
        setAutoRefresh,
        writeRegister,
        writeCoil,
    }
})
