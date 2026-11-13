import vcuCatalogJson from "./components/vcu.json"
import absCatalogJson from "./components/abs.json"
import bmsCatalogJson from "./components/bms.json"
import mcuCatalogJson from "./components/mcu.json"
import bleCatalogJson from "./components/ble.json"
import lightCatalogJson from "./components/light.json"
import tftCatalogJson from "./components/tft.json"
import { tauriApi } from "../services/tauri"
import { errorMessage } from "../services/protocol"
import type { ComponentCatalog, ComponentId } from "../types"

interface DeviceDefinition {
  name: string
  address: number
  fileName?: string
  catalog?: ComponentCatalog
  catalogError?: string
}

export const devices: DeviceDefinition[] = [
  { name: "BMS", address: bmsCatalogJson.deviceAddress, fileName: "bms.json", catalog: bmsCatalogJson },
  { name: "VCU", address: vcuCatalogJson.deviceAddress, fileName: "vcu.json", catalog: vcuCatalogJson },
  { name: "TSU", address: 3 },
  { name: "BLE", address: bleCatalogJson.deviceAddress, fileName: "ble.json", catalog: bleCatalogJson },
  { name: "MCU", address: mcuCatalogJson.deviceAddress, fileName: "mcu.json", catalog: mcuCatalogJson },
  { name: "ABS", address: absCatalogJson.deviceAddress, fileName: "abs.json", catalog: absCatalogJson },
  { name: "TBOX", address: 7 },
  { name: "TFT", address: tftCatalogJson.deviceAddress, fileName: "tft.json", catalog: tftCatalogJson },
  { name: "灯控盒", address: lightCatalogJson.deviceAddress, fileName: "light.json", catalog: lightCatalogJson },
]

const bundledCatalogs = new Map(devices.map((device) => [device.name, device.catalog]))

export function catalogFileName(id: ComponentId): string {
  const fileName = devices.find((device) => device.name === id)?.fileName
  if (!fileName) throw new Error(`部件 ${id} 没有状态目录`)
  return fileName
}

export function applyComponentCatalog(id: ComponentId, source: string): void {
  const device = devices.find((entry) => entry.name === id)
  if (!device?.fileName) throw new Error(`部件 ${id} 没有状态目录`)
  try {
    const catalog = JSON.parse(source) as ComponentCatalog
    device.catalog = catalog
    device.address = catalog.deviceAddress
    device.catalogError = undefined
  } catch (error) {
    device.catalog = undefined
    device.catalogError = `${device.fileName}：${errorMessage(error)}`
  }
}

export function resetComponentCatalog(id: ComponentId): void {
  const device = devices.find((entry) => entry.name === id)
  const catalog = bundledCatalogs.get(id)
  if (!device || !catalog) throw new Error(`部件 ${id} 没有内置状态目录`)
  device.catalog = catalog
  device.address = catalog.deviceAddress
  device.catalogError = undefined
}

export async function loadComponentCatalogs(): Promise<void> {
  await Promise.all(
    devices.filter((device) => device.fileName).map(async (device) => {
      try {
        const source = await tauriApi.readComponentCatalog(device.fileName!)
        if (source !== null) applyComponentCatalog(device.name as ComponentId, source)
      } catch (error) {
        device.catalog = undefined
        device.catalogError = `${device.fileName}：${errorMessage(error)}`
      }
    }),
  )
}

export function componentCatalog(id: ComponentId): ComponentCatalog {
  const device = devices.find((entry) => entry.name === id)
  if (device?.catalogError) throw new Error(device.catalogError)
  if (!device?.catalog) throw new Error(`部件 ${id} 没有状态目录`)
  return device.catalog
}
