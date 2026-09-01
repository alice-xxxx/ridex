import vcuCatalogJson from "./components/vcu.json"
import absCatalogJson from "./components/abs.json"
import bmsCatalogJson from "./components/bms.json"
import mcuCatalogJson from "./components/mcu.json"
import bleCatalogJson from "./components/ble.json"
import lightCatalogJson from "./components/light.json"
import tftCatalogJson from "./components/tft.json"
import type { ComponentCatalog, ComponentId } from "../types"

interface DeviceDefinition {
  name: string
  address: number
  catalog?: ComponentCatalog
}

export const devices: DeviceDefinition[] = [
  { name: "BMS", address: bmsCatalogJson.deviceAddress, catalog: bmsCatalogJson },
  { name: "VCU", address: vcuCatalogJson.deviceAddress, catalog: vcuCatalogJson },
  { name: "TSU", address: 3 },
  { name: "BLE", address: bleCatalogJson.deviceAddress, catalog: bleCatalogJson },
  { name: "MCU", address: mcuCatalogJson.deviceAddress, catalog: mcuCatalogJson },
  { name: "ABS", address: absCatalogJson.deviceAddress, catalog: absCatalogJson },
  { name: "TBOX", address: 7 },
  { name: "TFT", address: tftCatalogJson.deviceAddress, catalog: tftCatalogJson },
  { name: "灯控盒", address: lightCatalogJson.deviceAddress, catalog: lightCatalogJson },
]

export function componentCatalog(id: ComponentId): ComponentCatalog {
  const catalog = devices.find((device) => device.name === id)?.catalog
  if (!catalog) throw new Error(`部件 ${id} 没有状态目录`)
  return catalog
}
