<script setup lang="ts">
import { computed, onMounted, onScopeDispose, reactive } from "vue"
import { errorMessage } from "../services/protocol"
import { tauriApi } from "../services/tauri"
import type { FourGOtaEnvironment, FourGOtaFile, FourGOtaTask } from "../types"
import { displayFileName } from "../utils/displayFileName"

const props = defineProps<{ deviceId: string }>()

const state = reactive({
  loggedIn: false,
  loginBusy: false,
  credentialLoading: true,
  credentialsSaved: false,
  changingAccount: true,
  uploadBusy: false,
  startBusy: false,
  stopBusy: false,
  polling: false,
  fileUsername: "",
  filePassword: "",
  otaUsername: "",
  otaPassword: "",
  savedFileUsername: "",
  savedOtaUsername: "",
  filePath: "",
  firmwareUrl: "",
  uploadedFile: null as FourGOtaFile | null,
  targetSn: "G6XA24268RAAC3U9",
  environment: "prod" as FourGOtaEnvironment,
  otaType: 4,
  loopCount: 3,
  intervalSeconds: 20,
  task: null as FourGOtaTask | null,
  error: "",
})

let pollTimer: number | undefined
let disposed = false

const completedCount = computed(() => {
  if (!state.task) return 0
  return state.task.successCount + state.task.failureCount
})

const progress = computed(() => {
  if (!state.task?.totalCount) return 0
  return Math.min(100, Math.round((completedCount.value / state.task.totalCount) * 100))
})

const taskEnded = computed(() =>
  state.task ? ["COMPLETED", "STOPPED", "FAILED"].includes(state.task.status) : false,
)

const credentialsReady = computed(() => {
  const fileUsername = state.fileUsername.trim()
  const otaUsername = state.otaUsername.trim()
  const filePasswordReady =
    !!state.filePassword ||
    (state.credentialsSaved && fileUsername === state.savedFileUsername)
  const otaPasswordReady =
    !!state.otaPassword || (state.credentialsSaved && otaUsername === state.savedOtaUsername)
  return !!fileUsername && !!otaUsername && filePasswordReady && otaPasswordReady
})

const clearPoll = () => {
  if (pollTimer !== undefined) window.clearTimeout(pollTimer)
  pollTimer = undefined
  state.polling = false
}

const pollTask = async () => {
  if (disposed || !state.task || taskEnded.value) return clearPoll()
  state.polling = true
  try {
    state.task = await tauriApi.fourGOtaProgress(state.task.taskId)
    if (taskEnded.value) return clearPoll()
  } catch (cause) {
    state.error = errorMessage(cause, "读取 OTA 进度失败")
  }
  if (!disposed) pollTimer = window.setTimeout(pollTask, 1500)
}

const login = async () => {
  state.error = ""
  state.loginBusy = true
  try {
    await tauriApi.fourGOtaLogin(props.deviceId, {
      fileUsername: state.fileUsername.trim(),
      filePassword: state.filePassword,
      otaUsername: state.otaUsername.trim(),
      otaPassword: state.otaPassword,
    })
    state.loggedIn = true
    state.credentialsSaved = true
    state.changingAccount = false
    state.savedFileUsername = state.fileUsername.trim()
    state.savedOtaUsername = state.otaUsername.trim()
    state.filePassword = ""
    state.otaPassword = ""
  } catch (cause) {
    state.loggedIn = false
    state.error = errorMessage(cause, "4G OTA 平台登录失败")
  } finally {
    state.loginBusy = false
  }
}

onMounted(async () => {
  try {
    const credentials = await tauriApi.fourGOtaCredentials(props.deviceId)
    if (credentials.saved) {
      state.fileUsername = credentials.fileUsername ?? ""
      state.otaUsername = credentials.otaUsername ?? ""
      state.savedFileUsername = state.fileUsername
      state.savedOtaUsername = state.otaUsername
      state.credentialsSaved = true
      state.changingAccount = false
    }
  } catch (cause) {
    state.error = errorMessage(cause, "读取已保存账号失败")
  } finally {
    state.credentialLoading = false
  }
})

const changeAccount = () => {
  state.loggedIn = false
  state.changingAccount = true
  state.filePassword = ""
  state.otaPassword = ""
}

const cancelChangeAccount = () => {
  state.fileUsername = state.savedFileUsername
  state.otaUsername = state.savedOtaUsername
  state.filePassword = ""
  state.otaPassword = ""
  state.changingAccount = false
}

const chooseFile = async () => {
  state.error = ""
  const path = await tauriApi.pickFourGOtaFile()
  if (path) state.filePath = path
}

const upload = async () => {
  if (!state.filePath) return
  state.error = ""
  state.uploadBusy = true
  try {
    const file = await tauriApi.fourGOtaUpload(state.filePath)
    state.uploadedFile = file
    state.firmwareUrl = file.publicUrl
  } catch (cause) {
    state.error = errorMessage(cause, "上传 OTA 文件失败")
  } finally {
    state.uploadBusy = false
  }
}

const start = async () => {
  state.error = ""
  state.startBusy = true
  clearPoll()
  try {
    state.task = await tauriApi.fourGOtaStart({
      targetSn: state.targetSn.trim(),
      otaType: Number(state.otaType),
      loopCount: Number(state.loopCount),
      intervalSeconds: Number(state.intervalSeconds),
      firmwareUrl: state.firmwareUrl.trim(),
      environment: state.environment,
    })
    void pollTask()
  } catch (cause) {
    state.error = errorMessage(cause, "启动 4G OTA 失败")
  } finally {
    state.startBusy = false
  }
}

const stop = async () => {
  if (!state.task) return
  state.error = ""
  state.stopBusy = true
  try {
    state.task = await tauriApi.fourGOtaStop(state.task.taskId, state.task.environment)
    clearPoll()
  } catch (cause) {
    state.error = errorMessage(cause, "停止 4G OTA 失败")
  } finally {
    state.stopBusy = false
  }
}

onScopeDispose(() => {
  disposed = true
  clearPoll()
})
</script>

<template>
  <section class="four-g-panel">
    <div class="four-g-body">
      <section class="credential-block">
        <div class="block-title">
          <div>
            <strong>平台账号</strong>
            <small v-if="state.credentialLoading">正在读取已保存账号…</small>
            <small v-else-if="state.credentialsSaved && !state.changingAccount">
              已保存，可直接登录
            </small>
          </div>
        </div>
        <div v-if="state.credentialsSaved && !state.changingAccount" class="saved-accounts">
          <div>
            <span>文件平台</span>
            <strong>{{ state.savedFileUsername }}</strong>
          </div>
          <div>
            <span>OTA 平台</span>
            <strong>{{ state.savedOtaUsername }}</strong>
          </div>
        </div>
        <div v-else-if="!state.credentialLoading" class="credential-fields">
          <label class="field">
            <span>文件平台账号</span>
            <input v-model.trim="state.fileUsername" class="input input-sm" autocomplete="username" placeholder="必填" />
          </label>
          <label class="field">
            <span>文件平台密码</span>
            <input v-model="state.filePassword" class="input input-sm" type="password" autocomplete="current-password"
              placeholder="已保存" />
          </label>
          <label class="field">
            <span>OTA 平台账号</span>
            <input v-model.trim="state.otaUsername" class="input input-sm" autocomplete="username" placeholder="必填" />
          </label>
          <label class="field">
            <span>OTA 平台密码</span>
            <input v-model="state.otaPassword" class="input input-sm" type="password" autocomplete="current-password"
              placeholder="已保存" />
          </label>
        </div>
        <div class="credential-actions">
          <button v-if="state.credentialsSaved && !state.changingAccount" type="button" class="btn btn-ghost btn-sm"
            @click="changeAccount">
            更换账号
          </button>
          <button v-else-if="state.credentialsSaved" type="button" class="btn btn-ghost btn-sm"
            @click="cancelChangeAccount">
            取消更换
          </button>
          <button type="button" class="btn btn-sm" :class="state.loggedIn ? 'session-ready' : 'btn-primary'"
            :disabled="state.loginBusy || state.credentialLoading || !credentialsReady" @click="login">
            <span v-if="state.loginBusy" class="spinner"></span>
            {{ state.loginBusy ? "正在登录…" : state.loggedIn ? "重新登录" : "登录平台" }}
          </button>
        </div>
      </section>

      <section class="operation-block firmware-source">
        <div class="block-title">
          <strong>固件地址</strong>
          <small>上传文件，或填写已有地址</small>
        </div>
        <div class="file-row">
          <div class="file-copy">
            <span>本地 OTA 文件</span>
            <strong>{{ displayFileName(state.filePath) || "未选择" }}</strong>
          </div>
          <button class="btn btn-outline btn-sm" :disabled="state.uploadBusy" @click="chooseFile">
            选择文件
          </button>
          <button class="btn btn-primary btn-sm" :disabled="!state.loggedIn || !state.filePath || state.uploadBusy"
            @click="upload">
            {{ state.uploadBusy ? "上传中…" : "上传" }}
          </button>
        </div>
        <label class="field full-field">
          <span>平台文件地址</span>
          <input v-model.trim="state.firmwareUrl" class="input input-sm input-mono"
            placeholder="https://cdn.garow.com/.../firmware.ota" spellcheck="false" />
        </label>
        <small v-if="state.uploadedFile" class="upload-note">
          已上传 {{ state.uploadedFile.name }} · {{ state.uploadedFile.size }} 字节
        </small>
      </section>

      <section class="operation-block upgrade-config">
        <div class="block-title">
          <strong>升级参数</strong>
        </div>
        <div class="task-fields">
          <label class="field sn-field">
            <span>SN</span>
            <input v-model.trim="state.targetSn" class="input input-sm input-mono" />
          </label>
          <label class="field">
            <span>环境</span>
            <select v-model="state.environment" class="input input-sm">
              <option value="prod">生产环境</option>
              <option value="dev">开发环境</option>
            </select>
          </label>
          <label class="field">
            <span>OTA 类型</span>
            <select v-model.number="state.otaType" class="input input-sm">
              <option :value="0">云盒</option>
              <option :value="1">BMS</option>
              <option :value="2">仪表盘</option>
              <option :value="4">中控</option>
              <option :value="5">控制器</option>
              <option :value="6">ABS</option>
            </select>
          </label>
          <label class="field">
            <span>循环次数</span>
            <input v-model.number="state.loopCount" class="input input-sm input-mono" type="number" min="1" />
          </label>
          <label class="field">
            <span>间隔秒数</span>
            <input v-model.number="state.intervalSeconds" class="input input-sm input-mono" type="number" min="1" />
          </label>
        </div>
        <div class="start-row">
          <button class="btn btn-primary" :disabled="!state.loggedIn ||
            !state.targetSn.trim() ||
            !state.firmwareUrl.trim() ||
            state.startBusy ||
            (!!state.task && !taskEnded)
            " @click="start">
            {{ state.startBusy ? "正在启动…" : "执行 4G OTA" }}
          </button>
        </div>
      </section>

      <section v-if="state.task" class="task-status">
        <div class="task-head">
          <div>
            <span>任务 {{ state.task.taskId }}</span>
            <strong>{{ state.task.status }}</strong>
          </div>
          <span class="progress-value">{{ progress }}%</span>
        </div>
        <div class="progress-track" role="progressbar" :aria-valuenow="progress" aria-valuemin="0" aria-valuemax="100">
          <i :style="{ width: `${progress}%` }"></i>
        </div>
        <div class="task-metrics">
          <span>已发送 <b>{{ state.task.sentCount }}</b></span>
          <span>成功 <b>{{ state.task.successCount }}</b></span>
          <span>失败 <b>{{ state.task.failureCount }}</b></span>
          <span>{{ state.task.waitingReply ? "等待设备回复" : state.polling ? "同步任务状态" : "任务已结束" }}</span>
        </div>
        <div v-if="state.task.lastError" class="msg msg-error">{{ state.task.lastError }}</div>
        <button v-if="state.task.status === 'RUNNING'" class="btn btn-danger btn-sm stop-button"
          :disabled="state.stopBusy" @click="stop">
          {{ state.stopBusy ? "正在停止…" : "停止任务" }}
        </button>
      </section>

      <div v-if="state.error" class="msg msg-error">{{ state.error }}</div>
    </div>
  </section>
</template>

<style scoped>
.four-g-panel {
  min-height: 0;
  display: flex;
  flex: 1;
  flex-direction: column;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
  box-shadow: var(--shadow-sm);
}

.block-title {
  min-width: 0;
  display: flex;
  align-items: baseline;
  gap: 0.5625rem;
}

.block-title strong {
  color: var(--text-primary);
  font-size: 0.86rem;
}

.block-title small {
  color: var(--text-muted);
  font-size: 0.62rem;
}

.session-ready {
  border-color: rgba(22, 135, 103, 0.25);
  color: var(--success);
  background: var(--success-light);
}

.four-g-body {
  display: grid;
  flex: 1;
  min-height: 0;
  grid-template-columns: minmax(0, 0.9fr) minmax(0, 1.1fr);
  grid-template-rows: repeat(2, minmax(0, 1fr));
  align-content: stretch;
  gap: 0.75rem 1.125rem;
  padding: 0.875rem 1.0625rem 1.0625rem;
}

.operation-block {
  display: grid;
  align-content: space-between;
  gap: 0.625rem;
  padding: 0.75rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg);
}

.credential-block {
  display: grid;
  grid-column: 1;
  grid-row: 1;
  align-content: space-between;
  gap: 0.625rem;
  padding: 0.75rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
}

.firmware-source {
  grid-column: 1;
  grid-row: 2;
}

.upgrade-config {
  grid-column: 2;
  grid-row: 1 / span 2;
}

.task-status,
.four-g-body>.msg {
  grid-column: 1 / -1;
}

.credential-fields {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.5625rem;
}

.credential-block .block-title {
  justify-content: space-between;
}

.credential-block .block-title>div {
  min-width: 0;
  display: flex;
  align-items: baseline;
  gap: 0.5625rem;
}

.credential-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 0.5rem;
}

.saved-accounts {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.5625rem;
}

.saved-accounts>div {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 0.1875rem;
  padding: 0.5625rem 0.625rem;
  border-radius: var(--radius-xs);
  background: var(--success-light);
}

.saved-accounts span {
  color: var(--text-muted);
  font-size: 0.58rem;
}

.saved-accounts strong {
  overflow: hidden;
  color: var(--text-primary);
  font: 650 0.72rem/1.3 "Cascadia Code", monospace;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto;
  align-items: center;
  gap: 0.4375rem;
}

.file-copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.file-copy span,
.field>span {
  color: var(--text-secondary);
  font-size: 0.65rem;
  font-weight: 700;
}

.file-copy strong {
  overflow: hidden;
  color: var(--text-primary);
  font: 650 0.72rem/1.4 "Cascadia Code", monospace;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.field {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 0.3125rem;
}

.upload-note {
  color: var(--success);
  font-size: 0.62rem;
}

.task-fields {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.5625rem;
}

.sn-field {
  grid-column: 1 / -1;
}

.start-row {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 0.75rem;
}

.task-status {
  position: relative;
  padding: 0.8125rem;
  border: 1px solid rgba(0, 143, 149, 0.2);
  border-radius: var(--radius-sm);
  background: var(--primary-light);
}

.task-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
}

.task-head>div {
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.task-head span {
  overflow: hidden;
  color: var(--text-muted);
  font: 600 0.58rem/1.4 "Cascadia Code", monospace;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-head strong {
  color: var(--primary);
  font-size: 0.76rem;
}

.progress-value {
  color: var(--primary) !important;
  font-size: 1rem !important;
}

.progress-track {
  height: 6px;
  margin-top: 0.625rem;
  overflow: hidden;
  border-radius: 0.25rem;
  background: var(--border-strong);
}

.progress-track i {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: var(--primary);
  transition: width 0.25s ease;
}

.task-metrics {
  display: flex;
  flex-wrap: wrap;
  gap: 0.375rem 1rem;
  margin-top: 0.5625rem;
  color: var(--text-secondary);
  font-size: 0.63rem;
}

.task-metrics span:last-child {
  margin-left: auto;
  color: var(--text-muted);
}

.stop-button {
  margin-top: 0.625rem;
}

@media (max-width: 760px) {

  .four-g-body {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: none;
  }

  .credential-block,
  .firmware-source,
  .upgrade-config,
  .task-status,
  .four-g-body>.msg {
    grid-column: 1;
    grid-row: auto;
  }

  .block-title,
  .credential-block .block-title>div {
    align-items: flex-start;
    flex-direction: column;
    gap: 0.125rem;
  }

  .task-fields {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

}

@media (max-width: 480px) {

  .four-g-body {
    padding-left: 0.75rem;
    padding-right: 0.75rem;
  }

  .file-row {
    grid-template-columns: 1fr 1fr;
  }

  .file-copy {
    grid-column: 1 / -1;
  }

  .start-row {
    align-items: stretch;
    flex-direction: column;
  }

  .start-row .btn {
    width: 100%;
  }

  .task-metrics span:last-child {
    width: 100%;
    margin-left: 0;
  }
}
</style>
