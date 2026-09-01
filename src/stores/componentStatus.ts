import { computed, reactive, shallowRef, watch } from "vue"
import { defineStore } from "pinia"
import { componentCatalog } from "../registers/components"
import {
  encodeRegisterWrite,
  parseCoilValue,
  parseStatusData,
  runtimeItemKey,
} from "../registers/codec"
import { errorMessage, isDisconnected, isTimeout } from "../services/protocol"
import { tauriApi } from "../services/tauri"
import { useSessionStore } from "./session"
import { useTerminalStore } from "./terminal"
import type {
  ComponentCatalog,
  ReadBlock,
  StatusBlocks,
  StatusItem,
  StatusItemKind,
  StatusSnapshot,
} from "../types"

const AUTO_REFRESH_MS = 5000
const VERIFY_TIMEOUT_MS = 5000
const VERIFY_INTERVAL_MS = 250

type WritePlan =
  | {
    kind: "register"
    item: StatusItem
    optimisticItem: StatusItem
    expectedRawHex: string
    values: number[]
  }
  | {
    kind: "coil"
    item: StatusItem
    optimisticItem: StatusItem
    expectedRawHex: string
    values: boolean[]
  }

const sleep = (milliseconds: number) =>
  new Promise((resolve) => setTimeout(resolve, milliseconds))

function sameHex(left: string | null, right: string): boolean {
  return typeof left === "string" && left.toUpperCase() === right.toUpperCase()
}

function hasCompleteRawHex(item: StatusItem | null): boolean {
  return Boolean(
    item?.loaded &&
    Number.isInteger(item.bytes) &&
    item.bytes > 0 &&
    new RegExp(`^0x[0-9a-fA-F]{${item.bytes * 2}}$`).test(item.rawHex || ""),
  )
}

function blockLabel(kind: StatusItemKind, block: ReadBlock): string {
  const prefix = kind === "coil" ? "C" : "R"
  return `${prefix}${String(block.start).padStart(3, "0")}-${prefix}${String(block.start + block.count - 1).padStart(3, "0")}`
}

export const useComponentStatusStore = defineStore("component-status", () => {
  const session = useSessionStore()
  const terminal = useTerminalStore()
  const snapshot = shallowRef<StatusSnapshot>({ categories: [], registers: [], coils: [] })
  const pendingWrite = shallowRef<{ key: string; item: StatusItem } | null>(null)
  const reading = reactive({
    active: false,
    progress: { done: 0, total: 0 },
    error: "",
    automatic: true,
    lastAt: null as Date | null,
  })
  const writing = reactive({
    key: null as string | null,
    error: "",
  })

  let registerBlocks: StatusBlocks = {}
  let coilBlocks: StatusBlocks = {}
  let autoTimer: ReturnType<typeof setTimeout> | undefined
  let refreshGeneration = 0
  let writeGeneration = 0

  const data = computed(() => {
    const pending = pendingWrite.value
    if (!pending) return snapshot.value
    const withPendingWrite = (item: StatusItem) =>
      runtimeItemKey(item) === pending.key ? pending.item : item
    return {
      registers: snapshot.value.registers.map(withPendingWrite),
      coils: snapshot.value.coils.map(withPendingWrite),
      categories: snapshot.value.categories.map((category) => ({
        ...category,
        registers: category.registers.map(withPendingWrite),
        coils: category.coils.map(withPendingWrite),
      })),
    }
  })

  const inCooldown = computed(
    () => writing.key !== null || terminal.protocol.writing || terminal.ota.active,
  )

  function clearAutoTimer() {
    clearTimeout(autoTimer)
    autoTimer = undefined
  }

  function canRefresh(
    revision = session.runtime.revision,
    componentId = session.view.component,
  ): boolean {
    return (
      revision === session.runtime.revision &&
      componentId === session.view.component &&
      session.authenticated &&
      session.page === "status" &&
      !terminal.ota.active
    )
  }

  function reset() {
    clearAutoTimer()
    refreshGeneration += 1
    writeGeneration += 1
    registerBlocks = {}
    coilBlocks = {}
    snapshot.value = parseStatusData(registerBlocks, coilBlocks, componentCatalog(session.view.component))
    pendingWrite.value = null
    reading.active = false
    reading.progress = { done: 0, total: 0 }
    reading.error = ""
    reading.automatic = true
    reading.lastAt = null
    writing.key = null
    writing.error = ""
  }

  function scheduleAutoRefresh() {
    clearAutoTimer()
    if (!reading.automatic || !canRefresh()) return
    autoTimer = setTimeout(async () => {
      autoTimer = undefined
      await refresh()
      scheduleAutoRefresh()
    }, AUTO_REFRESH_MS)
  }

  function setAutoRefresh(enabled: boolean) {
    reading.automatic = enabled
    if (enabled) scheduleAutoRefresh()
    else clearAutoTimer()
  }

  async function requestBlock(
    kind: StatusItemKind,
    block: ReadBlock,
    sourceCatalog: ComponentCatalog,
  ): Promise<number[]> {
    const result =
      kind === "coil"
        ? await tauriApi.readCoils({
          device: sourceCatalog.deviceAddress,
          start: block.start,
          count: block.count,
        })
        : await tauriApi.readRegisters({
          device: sourceCatalog.deviceAddress,
          start: block.start,
          count: block.count,
        })
    const expectedLength = kind === "coil" ? Math.ceil(block.count / 8) : block.count * 2
    if (!Array.isArray(result.data)) throw new Error(`${blockLabel(kind, block)} 响应不含数据`)
    if (
      (kind === "register" && result.data.length !== expectedLength) ||
      (kind === "coil" && result.data.length < expectedLength)
    ) {
      throw new Error(
        `${blockLabel(kind, block)} 响应长度错误：期望${expectedLength}字节，收到${result.data.length}字节`,
      )
    }
    return result.data.slice(0, expectedLength)
  }

  function handleTransportError(error: unknown) {
    if (isTimeout(error)) reading.automatic = false
    session.handleDisconnected(error)
  }

  async function refresh(): Promise<boolean> {
    if (!canRefresh() || reading.active || writing.key !== null || terminal.protocol.writing) {
      return false
    }
    clearAutoTimer()
    const revision = session.runtime.revision
    const componentId = session.view.component
    const sourceCatalog = componentCatalog(componentId)

    const generation = ++refreshGeneration
    const blocks: Array<[StatusItemKind, ReadBlock]> = [
      ...sourceCatalog.readBlocks.map(
        (block): [StatusItemKind, ReadBlock] => ["register", block],
      ),
      ...sourceCatalog.coilReadBlocks.map(
        (block): [StatusItemKind, ReadBlock] => ["coil", block],
      ),
    ]
    const nextRegisterBlocks = { ...registerBlocks }
    const nextCoilBlocks = { ...coilBlocks }
    const failures: string[] = []
    let completedBlocks = 0
    let stoppedByTimeout = false

    reading.active = true
    reading.progress = { done: 0, total: blocks.length }
    reading.error = ""
    writing.error = ""
    session.runtime.statusActive = true

    try {
      for (const [kind, block] of blocks) {
        if (!canRefresh(revision, componentId) || generation !== refreshGeneration) return false
        try {
          const bytes = await requestBlock(kind, block, sourceCatalog)
          if (!canRefresh(revision, componentId) || generation !== refreshGeneration) return false
          if (kind === "coil") nextCoilBlocks[block.start] = bytes
          else nextRegisterBlocks[block.start] = bytes
          completedBlocks += 1
        } catch (error) {
          if (!canRefresh(revision, componentId) || generation !== refreshGeneration) return false
          handleTransportError(error)
          if (isDisconnected(error)) return false
          failures.push(`${blockLabel(kind, block)}：${errorMessage(error)}`)
          if (isTimeout(error)) {
            stoppedByTimeout = true
            break
          }
        } finally {
          if (generation === refreshGeneration) reading.progress.done += 1
        }
      }

      if (!canRefresh(revision, componentId) || generation !== refreshGeneration) return false
      registerBlocks = nextRegisterBlocks
      coilBlocks = nextCoilBlocks
      snapshot.value = parseStatusData(registerBlocks, coilBlocks, sourceCatalog)
      if (completedBlocks > 0) reading.lastAt = new Date()
      if (failures.length) {
        const prefix = stoppedByTimeout ? "读取超时，自动刷新已停止" : "部分数据读取失败"
        reading.error = `${prefix}\n${failures.map((message) => `• ${message}`).join("\n")}`
      }
      return failures.length === 0
    } catch (error) {
      handleTransportError(error)
      if (
        generation === refreshGeneration &&
        revision === session.runtime.revision &&
        componentId === session.view.component
      ) {
        reading.error = errorMessage(error)
      }
      return false
    } finally {
      if (generation === refreshGeneration) {
        reading.active = false
        session.runtime.statusActive = false
        scheduleAutoRefresh()
      }
    }
  }

  function findBlock(
    kind: StatusItemKind,
    address: number,
    sourceCatalog: ComponentCatalog,
  ): ReadBlock | null {
    const blocks = kind === "coil" ? sourceCatalog.coilReadBlocks : sourceCatalog.readBlocks
    return blocks.find(({ start, count }) => address >= start && address < start + count) ?? null
  }

  function currentItem(kind: StatusItemKind, address: number): StatusItem | null {
    const items = kind === "coil" ? snapshot.value.coils : snapshot.value.registers
    return items.find((item) => item.address === address) ?? null
  }

  async function verify(
    plan: WritePlan,
    revision: number,
    generation: number,
    componentId: typeof session.view.component,
    sourceCatalog: ComponentCatalog,
  ): Promise<boolean> {
    const block = findBlock(plan.kind, plan.item.address, sourceCatalog)
    if (!block) {
      throw new Error(
        `${plan.kind === "coil" ? "线圈" : "寄存器"} ${plan.item.address} 不在读取范围内`,
      )
    }

    const deadline = Date.now() + VERIFY_TIMEOUT_MS
    let lastReadError: unknown = null
    do {
      if (
        revision !== session.runtime.revision ||
        componentId !== session.view.component ||
        generation !== writeGeneration
      ) {
        return false
      }
      try {
        const bytes = await requestBlock(plan.kind, block, sourceCatalog)
        if (plan.kind === "coil") coilBlocks = { ...coilBlocks, [block.start]: bytes }
        else registerBlocks = { ...registerBlocks, [block.start]: bytes }
        snapshot.value = parseStatusData(registerBlocks, coilBlocks, sourceCatalog)
        const actual = currentItem(plan.kind, plan.item.address)
        if (actual?.loaded && sameHex(actual.rawHex, plan.expectedRawHex)) return true
        lastReadError = null
      } catch (error) {
        if (isDisconnected(error)) throw error
        handleTransportError(error)
        lastReadError = error
      }
      if (Date.now() < deadline) {
        await sleep(Math.min(VERIFY_INTERVAL_MS, deadline - Date.now()))
      }
    } while (Date.now() < deadline)

    if (lastReadError) throw lastReadError
    return false
  }

  function writeBlockedMessage(): string {
    if (!session.authenticated || session.page !== "status") return "当前没有可写入的部件状态"
    if (terminal.ota.active) return "OTA 更新中，状态写入已暂停"
    if (writing.key !== null) return "已有状态正在写入"
    if (terminal.protocol.writing) return "已有协议操作正在执行"
    return ""
  }

  async function commitWrite(plan: WritePlan, sourceCatalog: ComponentCatalog): Promise<boolean> {
    const componentId = session.view.component
    const revision = session.runtime.revision
    const generation = ++writeGeneration
    const key = runtimeItemKey(plan.item)

    pendingWrite.value = { key, item: plan.optimisticItem }
    writing.key = key
    writing.error = ""
    reading.error = ""
    session.runtime.statusActive = true
    clearAutoTimer()

    try {
      if (plan.kind === "register") {
        await tauriApi.writeRegisters({
          device: sourceCatalog.deviceAddress,
          start: plan.item.address,
          values: plan.values,
        })
      } else {
        await tauriApi.writeCoils({
          device: sourceCatalog.deviceAddress,
          start: plan.item.address,
          values: plan.values,
        })
      }
      if (!(await verify(plan, revision, generation, componentId, sourceCatalog))) {
        throw new Error(
          `写入后未确认 ${plan.kind === "coil" ? "C" : "R"}${plan.item.address} 的目标值`,
        )
      }
      return true
    } catch (error) {
      handleTransportError(error)
      if (
        revision === session.runtime.revision &&
        componentId === session.view.component &&
        generation === writeGeneration
      ) {
        writing.error = errorMessage(error)
      }
      return false
    } finally {
      if (generation === writeGeneration) {
        pendingWrite.value = null
        writing.key = null
        session.runtime.statusActive = false
        scheduleAutoRefresh()
      }
    }
  }

  async function writeRegister(address: number, contentIndex: number, value: unknown) {
    const blocked = writeBlockedMessage()
    if (blocked) {
      writing.error = blocked
      return false
    }
    const item = currentItem("register", address)
    if (!item || !hasCompleteRawHex(item)) {
      writing.error = `寄存器 R${address} 尚未完整读取，不能写入`
      return false
    }
    const sourceCatalog = componentCatalog(session.view.component)

    try {
      const encoded = encodeRegisterWrite(item, contentIndex, value, sourceCatalog)
      return commitWrite(
        {
          kind: "register",
          item,
          optimisticItem: encoded.optimisticItem,
          expectedRawHex: encoded.expectedRawHex,
          values: encoded.values,
        },
        sourceCatalog,
      )
    } catch (error) {
      writing.error = errorMessage(error, "写入值无效")
      return false
    }
  }

  async function writeCoil(address: number, value: boolean | number) {
    const blocked = writeBlockedMessage()
    if (blocked) {
      writing.error = blocked
      return false
    }
    const item = currentItem("coil", address)
    if (!item || !hasCompleteRawHex(item)) {
      writing.error = `线圈 C${address} 尚未完整读取，不能写入`
      return false
    }
    if (![true, false, 0, 1].includes(value)) {
      writing.error = "线圈写入值必须是布尔值"
      return false
    }
    const sourceCatalog = componentCatalog(session.view.component)
    const active = Boolean(value)
    const optimisticItem = parseCoilValue(address, active, sourceCatalog)
    return commitWrite(
      {
        kind: "coil",
        item,
        optimisticItem,
        expectedRawHex: optimisticItem.rawHex ?? "",
        values: [active],
      },
      sourceCatalog,
    )
  }

  watch([() => session.runtime.revision, () => session.view.component], reset, {
    immediate: true,
    flush: "sync",
  })
  watch(
    [
      () => session.authenticated,
      () => session.page,
      () => session.view.component,
      () => terminal.ota.active,
      () => terminal.protocol.writing,
    ],
    () => {
      clearAutoTimer()
      if (
        !session.authenticated ||
        session.page !== "status" ||
        terminal.ota.active ||
        terminal.protocol.writing
      ) {
        return
      }
      const hasLoadedItem =
        snapshot.value.registers.some((item) => item.loaded) ||
        snapshot.value.coils.some((item) => item.loaded)
      if (!hasLoadedItem && reading.automatic) void refresh()
      else scheduleAutoRefresh()
    },
    { immediate: true },
  )

  return {
    data,
    reading,
    writing,
    inCooldown,
    refresh,
    setAutoRefresh,
    writeRegister,
    writeCoil,
  }
})
