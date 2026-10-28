export type Transport = "ble" | "serial" | "network"
export type NetworkPlatform = "production" | "test"
export type ActiveView = "search" | "auth" | "status" | "terminal" | "ota-tool"
export type ComponentId = "VCU" | "BMS" | "BLE" | "MCU" | "ABS" | "TFT" | "灯控盒"

export interface Connection {
  transport: Transport
  endpoint: string
  name: string
  authenticated: boolean
  supportsOta: boolean
}

export interface DiscoveredEndpoint {
  transport: "ble" | "serial"
  id: string
  name: string
  signal: number | null
}

interface BleDiscoverRequest {
  transport: "ble"
  timeoutMs: number
  serviceUuid: string | null
}

interface SerialDiscoverRequest {
  transport: "serial"
}

export type DiscoverRequest = BleDiscoverRequest | SerialDiscoverRequest

interface BleConnectRequest {
  transport: "ble"
  address: string
  name: string
  timeoutMs: number
}

interface SerialConnectRequest {
  transport: "serial"
  path: string
  baudRate: number
}

interface NetworkConnectRequest {
  transport: "network"
  name: string
  platform: NetworkPlatform
  username: string
  password: string
  timeoutMs: number
}

export type ConnectRequest = BleConnectRequest | SerialConnectRequest | NetworkConnectRequest

export interface NetworkCredentials {
  saved: boolean
  username: string | null
}

export interface ReadModbusParam {
  device: number
  start: number
  count: number
}

export interface WriteRegistersParam {
  device: number
  start: number
  values: number[]
}

export interface WriteCoilsParam {
  device: number
  start: number
  values: boolean[]
}

export type ModbusRequest =
  | ({ operation: "readRegisters" | "readCoils" } & ReadModbusParam)
  | ({ operation: "writeRegisters" } & WriteRegistersParam)
  | ({ operation: "writeCoils" } & WriteCoilsParam)

export interface ModbusResponse {
  request: number[]
  response: number[]
  data: number[]
}

export interface LicenseStatus {
  authorized: boolean
  deviceId: string
  message: string | null
}

export interface AppUpdate {
  version: string
  url: string
}

export interface OtaFileInfo {
  fileCrc: number
  fileSize: number
  manufacturer: number[]
  deviceType: number[]
  hardwareVersion: number[]
  softwareVersion: number[]
  deviceAddress: number | null
  description: string
  upgradeCrc: number
  upgradeSize: number
}

export interface OtaMetadata {
  manufacturer: number[]
  deviceType: number[]
  hardwareVersion: number[]
  softwareVersion: number[]
  deviceAddress: number
  description: string
}

export interface OtaPackRequest {
  inputPath: string
  outputPath: string
  request: OtaMetadata
}

export type OtaFileKind = "binary" | "packaged"

export interface OtaRequest {
  path: string
  isOta: boolean
  request: OtaMetadata
  cycles: number
}

export interface OtaStartResult {
  bytesPerCycle: number
}

export type FourGOtaEnvironment = "dev" | "prod"

export interface FourGOtaLoginRequest {
  fileUsername: string
  filePassword: string
  otaUsername: string
  otaPassword: string
}

export interface FourGOtaCredentialStatus {
  saved: boolean
  fileUsername: string | null
  otaUsername: string | null
}

export interface FourGOtaFile {
  id: string
  name: string
  relativePath: string
  urlPath: string
  publicUrl: string
  fileType: string
  mime: string
  size: number
  convertedWebP: boolean
  createdAt: string
}

export interface FourGOtaTask {
  taskId: string
  targetSn: string
  firmwareUrl: string
  otaType: number
  patch: string | null
  environment: FourGOtaEnvironment
  status: string
  totalCount: number
  sentCount: number
  successCount: number
  failureCount: number
  intervalSeconds: number | null
  lastSerialNumber: number | null
  waitingReply: boolean
  lastReplyStatus: string | null
  lastReplyAt: string | null
  startedAt: string | null
  nextRunAt: string | null
  finishedAt: string | null
  lastError: string | null
  createdAt: string | null
  updatedAt: string | null
}

export interface FourGOtaStartRequest {
  targetSn: string
  otaType: number
  loopCount: number
  intervalSeconds: number
  firmwareUrl: string
  environment: FourGOtaEnvironment
}

export interface ConnectionClosedEvent {
  transport: Transport
  reason: string
}

export type BitRange = number[]

interface ContentOption {
  value: string | number | boolean
  label: string
}

export interface ContentDefinition {
  kind: string
  label?: string
  codec?: string
  value?: unknown
  unit?: string
  min?: number
  max?: number
  scale?: number
  precision?: number
  bits?: BitRange[]
  writeBits?: BitRange[]
  writePrefixHex?: string
  verifyValue?: number
  options?: ContentOption[]
  on?: string
  off?: string
  step?: number | string
  maxLength?: number
  multiplierBase?: number
  magnitudeBits?: BitRange[]
  signBits?: BitRange[]
  multiplierBits?: BitRange[]
  note?: string
}

export interface ReadBlock {
  start: number
  count: number
}

export interface RegisterDefinition {
  bytes: number
  label: string
  writable?: boolean
  writePrefixHex?: string
  content: ContentDefinition[]
}

interface CategoryDefinition {
  name: string
  registers?: number[]
  coils?: number[]
}

export interface ComponentCatalog {
  readBlocks: ReadBlock[]
  coilReadBlocks: ReadBlock[]
  categories: Record<string, CategoryDefinition>
  registers: Record<string, RegisterDefinition>
  coils: Record<string, RegisterDefinition>
  deviceAddress: number
}

export type StatusItemKind = "register" | "coil"

export interface StatusItem {
  address: number
  kind: StatusItemKind
  loaded: boolean
  bytes: number
  label: string
  writable: boolean
  rawHex: string | null
  writePrefixHex?: string
  content: ContentDefinition[]
}

export interface StatusCategory {
  id: string
  name: string
  registers: StatusItem[]
  coils: StatusItem[]
}

export interface StatusSnapshot {
  categories: StatusCategory[]
  registers: StatusItem[]
  coils: StatusItem[]
}

export type StatusBlocks = Record<number, number[]>

export interface TransactionState {
  request: string
  raw: string
  data: string
  error: string
}

export interface CommunicationLog extends TransactionState {
  id: number
  time: string
  type: "error" | "success"
  label: string
}

export interface OtaResultMessage {
  type: "success" | "warn" | "error"
  message: string
}
