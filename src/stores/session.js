import { computed, ref, shallowRef } from "vue"
import { defineStore } from "pinia"
import { errorMessage, isDisconnected } from "../services/protocol.js"
import { tauriApi } from "../services/tauri.js"

export const DEFAULT_VEHICLE_CODE = "13852042434"
export const SAVED_NETWORK_PASSWORD = "__ridex_saved_password__"

export const useSessionStore = defineStore("session", () => {
    const connection = shallowRef(null)
    const endpoints = shallowRef([])
    const accessMode = ref("ble")
    const networkPlatform = ref("production")
    const networkUsername = ref("")
    const networkPassword = ref("")
    const networkCredentials = ref({ saved: false, username: null })
    const networkCredentialsLoading = ref(false)
    const component = ref("vehicle")
    const activeView = ref("status")
    const revision = ref(0)
    const statusOperationActive = ref(false)

    const serialSupported = ref(false)
    const scanning = ref(false)
    const scanError = ref("")
    const connecting = ref(false)
    const connectingEndpoint = ref("")
    const connectError = ref("")
    const disconnecting = ref(false)
    const disconnectError = ref("")
    const authenticating = ref(false)
    const vehicleCode = ref(DEFAULT_VEHICLE_CODE)
    const authError = ref("")

    const connected = computed(() => connection.value !== null)
    const authenticated = computed(() => connection.value?.authenticated === true)
    const transport = computed(() => connection.value?.transport ?? accessMode.value)
    const connectedAddress = computed(() => connection.value?.endpoint ?? "")
    const connectedName = computed(() => connection.value?.name ?? "")
    const supportsOta = computed(() => connection.value?.supportsOta === true)
    const page = computed(() => {
        if (activeView.value === "ota-tool") return "ota-tool"
        if (!connection.value) return "search"
        if (!connection.value.authenticated) return "auth"
        return activeView.value
    })

    function reset(reason = "") {
        revision.value += 1
        connection.value = null
        component.value = "vehicle"
        activeView.value = "status"
        statusOperationActive.value = false
        authenticating.value = false
        connecting.value = false
        connectingEndpoint.value = ""
        scanError.value = ""
        connectError.value = reason
        disconnectError.value = ""
        authError.value = ""
    }

    function handleDisconnected(error) {
        if (!isDisconnected(error)) return false
        reset(transport.value === "ble" ? "蓝牙连接已断开" : "连接已断开")
        return true
    }

    const connectionEventsReady = tauriApi
        .onConnectionClosed(({ reason }) => reset(reason || "连接已断开"))
        .then(() => true)
        .catch((error) => {
            connectError.value = errorMessage(error, "连接事件监听启动失败")
            return false
        })

    async function loadChannelCaps() {
        try {
            serialSupported.value = await tauriApi.channelCaps()
        } catch {
            serialSupported.value = false
        }
        if (!serialSupported.value && accessMode.value === "serial") accessMode.value = "ble"
    }

    async function selectNetworkPlatform(platform) {
        networkPlatform.value = platform
        networkUsername.value = ""
        networkPassword.value = ""
        connectError.value = ""
        await loadNetworkCredentials(platform)
    }

    async function loadNetworkCredentials(platform = networkPlatform.value) {
        networkCredentialsLoading.value = true
        try {
            const status = await tauriApi.networkCredentials(platform)
            networkCredentials.value = status
            networkUsername.value = status.username ?? ""
            networkPassword.value = status.saved ? SAVED_NETWORK_PASSWORD : ""
        } catch {
            networkCredentials.value = { saved: false, username: null }
            networkUsername.value = ""
            networkPassword.value = ""
        } finally {
            networkCredentialsLoading.value = false
        }
    }

    async function discover(transportType = accessMode.value) {
        if (scanning.value || disconnecting.value) return
        scanning.value = true
        scanError.value = ""
        connectError.value = ""
        disconnectError.value = ""
        try {
            endpoints.value = await tauriApi.discover(transportType === "ble"
                ? { transport: "ble", timeoutMs: 3000, serviceUuid: null }
                : { transport: "serial" })
        } catch (error) {
            scanError.value = errorMessage(error)
            handleDisconnected(error)
        } finally {
            scanning.value = false
        }
    }

    async function connect(request) {
        if (connecting.value || disconnecting.value) return false
        const requestToSend = request.transport === "network"
            && request.password === SAVED_NETWORK_PASSWORD
            ? { ...request, username: "", password: "" }
            : request
        connecting.value = true
        connectingEndpoint.value = requestToSend.address ?? requestToSend.path ?? requestToSend.name ?? ""
        scanError.value = ""
        connectError.value = ""
        disconnectError.value = ""
        authError.value = ""
        accessMode.value = requestToSend.transport
        component.value = "vehicle"
        if (requestToSend.platform) networkPlatform.value = requestToSend.platform
        try {
            if (!await connectionEventsReady) throw new Error("连接事件监听未就绪")
            connection.value = await tauriApi.connect(requestToSend)
            if (requestToSend.transport === "network") {
                networkUsername.value = ""
                networkPassword.value = ""
            }
            activeView.value = "status"
            return true
        } catch (error) {
            connectError.value = errorMessage(error)
            connection.value = null
            return false
        } finally {
            connecting.value = false
            connectingEndpoint.value = ""
        }
    }

    async function auth(code = vehicleCode.value) {
        if (authenticating.value) return false
        authenticating.value = true
        authError.value = ""
        connectError.value = ""
        disconnectError.value = ""
        try {
            await tauriApi.channelAuth(code)
            connection.value = { ...connection.value, authenticated: true }
            activeView.value = "status"
            return true
        } catch (error) {
            authError.value = errorMessage(error)
            handleDisconnected(error)
            return false
        } finally {
            authenticating.value = false
        }
    }

    async function disconnect() {
        if (!connection.value || disconnecting.value) return
        disconnecting.value = true
        disconnectError.value = ""
        reset()
        try {
            await tauriApi.disconnect()
        } catch (error) {
            connectError.value = errorMessage(error)
        } finally {
            disconnecting.value = false
        }
    }

    function goTo(view) {
        if (view === "search") {
            activeView.value = view
            return
        }
        if (view === "ota-tool") {
            activeView.value = view
            return
        }
        if (view === "auth" && connection.value && !authenticated.value) {
            activeView.value = view
            return
        }
        if (authenticated.value && (view === "status" || view === "terminal")) {
            activeView.value = view
        }
    }

    function selectComponent(nextComponent) {
        component.value = nextComponent || "vehicle"
    }

    function rssiLevel(rssi) {
        if (rssi >= -40) return "excellent"
        if (rssi >= -60) return "good"
        if (rssi >= -80) return "fair"
        return "weak"
    }

    return {
        connection, endpoints, accessMode, networkPlatform, networkUsername, networkPassword,
        networkCredentials, networkCredentialsLoading, component,
        activeView, revision, statusOperationActive,
        serialSupported, scanning, scanError, connecting, connectingEndpoint, connectError,
        disconnecting, disconnectError, authenticating, vehicleCode, authError,
        connected, authenticated, transport, connectedAddress, connectedName, supportsOta, page,
        loadChannelCaps, selectNetworkPlatform, loadNetworkCredentials,
        discover, connect, auth, disconnect, goTo, selectComponent,
        handleDisconnected, rssiLevel,
    }
})
