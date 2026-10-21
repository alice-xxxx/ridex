import { computed, onScopeDispose, reactive } from "vue"
import { defineStore } from "pinia"
import { errorMessage, isDisconnected } from "../services/protocol"
import { tauriApi } from "../services/tauri"
import type {
  ActiveView,
  ComponentId,
  Connection,
  ConnectRequest,
  DiscoveredEndpoint,
  NetworkPlatform,
  Transport,
} from "../types"

const DEFAULT_VEHICLE_CODE = "13852042434"

export const useSessionStore = defineStore("session", () => {
  const access = reactive({
    mode: "ble" as Transport,
    endpoints: [] as DiscoveredEndpoint[],
    serialSupported: false,
    scanning: false,
    error: "",
  })
  const link = reactive({
    current: null as Connection | null,
    connecting: false,
    endpoint: "",
    connectError: "",
    disconnecting: false,
  })
  const authorization = reactive({
    active: false,
    vehicleCode: DEFAULT_VEHICLE_CODE,
    error: "",
  })
  const view = reactive({
    active: "status" as ActiveView,
    component: "VCU" as ComponentId,
  })
  const runtime = reactive({
    revision: 0,
    statusActive: false,
  })

  const authenticated = computed(() => link.current?.authenticated === true)
  const transport = computed(() => link.current?.transport ?? access.mode)
  const supportsOta = computed(() => link.current?.supportsOta === true)
  const page = computed<ActiveView>(() => {
    if (view.active === "ota-tool") return "ota-tool"
    if (!link.current) return "search"
    if (!link.current.authenticated) return "auth"
    return view.active
  })

  function reset(reason = "") {
    runtime.revision += 1
    runtime.statusActive = false
    link.current = null
    link.connecting = false
    link.endpoint = ""
    link.connectError = reason
    access.error = ""
    authorization.active = false
    authorization.error = ""
    view.active = "status"
    view.component = "VCU"
  }

  function handleDisconnected(error: unknown): boolean {
    if (!isDisconnected(error)) return false
    reset(transport.value === "ble" ? "蓝牙连接已断开" : "连接已断开")
    return true
  }

  let removeConnectionListener: (() => void) | undefined
  const connectionEventsReady = tauriApi
    .onConnectionClosed(({ reason }) => reset(reason || "连接已断开"))
    .then((unlisten) => {
      removeConnectionListener = unlisten
      return true
    })
    .catch((error) => {
      link.connectError = errorMessage(error, "连接事件监听启动失败")
      return false
    })
  onScopeDispose(() => removeConnectionListener?.())

  function loadSerialSupport() {
    access.serialSupported = tauriApi.supportsSerial()
    if (!access.serialSupported && access.mode === "serial") access.mode = "ble"
  }

  async function loadNetworkCredentials(platform: NetworkPlatform) {
    try {
      const credentials = await tauriApi.networkCredentials(platform)
      return {
        saved: credentials.saved,
        username: credentials.username ?? "",
      }
    } catch {
      return { saved: false, username: "" }
    }
  }

  async function discover(transportType: DiscoveredEndpoint["transport"]) {
    if (access.scanning || link.disconnecting) return
    access.scanning = true
    access.error = ""
    link.connectError = ""
    try {
      access.endpoints = await tauriApi.discover(
        transportType === "ble"
          ? { transport: "ble", timeoutMs: 3000, serviceUuid: null }
          : { transport: "serial" },
      )
    } catch (error) {
      access.error = errorMessage(error)
      handleDisconnected(error)
    } finally {
      access.scanning = false
    }
  }

  async function connect(request: ConnectRequest): Promise<boolean> {
    if (link.connecting || link.disconnecting) return false

    link.connecting = true
    if (request.transport === "ble") link.endpoint = request.address
    else if (request.transport === "serial") link.endpoint = request.path
    else link.endpoint = request.name
    link.connectError = ""
    access.error = ""
    authorization.error = ""
    access.mode = request.transport
    view.component = "VCU"

    try {
      if (!(await connectionEventsReady)) throw new Error("连接事件监听未就绪")
      link.current = await tauriApi.connect(request)
      view.active = "status"
      return true
    } catch (error) {
      link.connectError = errorMessage(error)
      link.current = null
      return false
    } finally {
      link.connecting = false
      link.endpoint = ""
    }
  }

  async function authenticate(code = authorization.vehicleCode): Promise<boolean> {
    if (authorization.active || !link.current) return false
    authorization.active = true
    authorization.error = ""
    link.connectError = ""
    try {
      await tauriApi.authenticate(code)
      link.current.authenticated = true
      view.active = "status"
      return true
    } catch (error) {
      authorization.error = errorMessage(error)
      handleDisconnected(error)
      return false
    } finally {
      authorization.active = false
    }
  }

  async function disconnect() {
    if (!link.current || link.disconnecting) return
    link.disconnecting = true
    reset()
    try {
      await tauriApi.disconnect()
    } catch (error) {
      link.connectError = errorMessage(error)
    } finally {
      link.disconnecting = false
    }
  }

  function goTo(next: ActiveView) {
    if (next === "search" || next === "ota-tool") {
      view.active = next
    } else if (next === "auth" && link.current && !authenticated.value) {
      view.active = next
    } else if (authenticated.value && (next === "status" || next === "terminal")) {
      view.active = next
    }
  }

  return {
    access,
    link,
    authorization,
    view,
    runtime,
    authenticated,
    transport,
    supportsOta,
    page,
    loadSerialSupport,
    loadNetworkCredentials,
    discover,
    connect,
    authenticate,
    disconnect,
    goTo,
    handleDisconnected,
  }
})
