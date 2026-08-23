import vehicleCatalog from "./components/vcu.json" with { type: "json" }
import abs from "./components/abs.json" with { type: "json" }
import bms from "./components/bms.json" with { type: "json" }
import controller from "./components/controller.json" with { type: "json" }
import ble from "./components/ble.json" with { type: "json" }

const vehicle = Object.freeze({
    ...vehicleCatalog,
    id: "vehicle",
    name: "VCU 车辆状态",
    deviceAddress: 2,
})

export const deviceAddresses = Object.freeze([
    { value: 1, label: "BMS" },
    { value: 2, label: "VCU" },
    { value: 3, label: "TSU" },
    { value: 4, label: "BLE" },
    { value: 5, label: "MCU" },
    { value: 6, label: "ABS" },
    { value: 7, label: "TBOX" },
    { value: 9, label: "TFT" },
])

export const componentDefinitions = Object.freeze({
    vehicle: { id: "vehicle", name: "VCU 车辆状态", deviceAddress: 2 },
    bms: { id: "bms", name: "BMS 电池管理", deviceAddress: 1 },
    ble: { id: "ble", name: "BLE 报警器", deviceAddress: 4 },
    controller: { id: "controller", name: "MCU 控制器", deviceAddress: 5 },
    abs: { id: "abs", name: "ABS 防抱死", deviceAddress: 6 },
})

export const componentCatalogs = Object.freeze({
    vehicle,
    bms,
    ble,
    controller,
    abs,
})

export function componentCatalog(id) {
    return componentCatalogs[id] ?? null
}

export function componentDefinition(id) {
    return componentDefinitions[id] ?? componentDefinitions.vehicle
}
