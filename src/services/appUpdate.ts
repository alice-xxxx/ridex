import { watch } from "vue"
import { ask } from "@tauri-apps/plugin-dialog"
import { platform } from "@tauri-apps/plugin-os"
import { tauriApi } from "./tauri"
import { useTerminalStore } from "../stores/terminal"

async function savePackage(message: string) {
  const confirmed = await ask(message, {
    title: "软件更新", kind: "info", okLabel: "保存安装包", cancelLabel: "稍后",
  })
  if (confirmed) await tauriApi.saveAppUpdate()
}

export async function checkForAppUpdate() {
  try {
    const update = await tauriApi.checkAppUpdate()
    if (!update) return
    await tauriApi.downloadAppUpdate(update)

    const offerUpdate = async () => {
      try {
        if (!["windows", "android", "macos"].includes(platform())) {
          await savePackage(`新版本 ${update.version} 已下载完成。当前平台不支持直接安装，是否保存安装包？`)
          return
        }
        const confirmed = await ask(`新版本 ${update.version} 已下载完成，是否现在安装？`, {
          title: "软件更新", kind: "info", okLabel: "安装", cancelLabel: "稍后",
        })
        if (confirmed) {
          try { await tauriApi.installAppUpdate() } catch {
            await savePackage("无法打开安装器，是否保存已下载的安装包？")
          }
        }
      } catch { /* Keep updates unobtrusive. */ }
    }

    const terminal = useTerminalStore()
    if (terminal.ota.active) {
      watch(() => terminal.ota.active, () => { void offerUpdate() }, { once: true })
    } else {
      await offerUpdate()
    }
  } catch {
    // Network, server and download failures never interrupt normal app use.
  }
}
