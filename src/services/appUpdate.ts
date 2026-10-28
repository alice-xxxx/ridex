import { watch } from "vue"
import { ask } from "@tauri-apps/plugin-dialog"
import { platform } from "@tauri-apps/plugin-os"
import { tauriApi } from "./tauri"
import { useTerminalStore } from "../stores/terminal"

async function savePackage() {
  try { await tauriApi.saveAppUpdate() } catch { /* Keep updates unobtrusive. */ }
}

export async function checkForAppUpdate() {
  try {
    const update = await tauriApi.checkAppUpdate()
    if (!update) return
    await tauriApi.downloadAppUpdate(update)

    if (!["windows", "android", "macos"].includes(platform())) {
      await savePackage()
      return
    }

    const offerInstall = async () => {
      try {
        const confirmed = await ask(`新版本 ${update.version} 已下载完成，是否现在安装？`, {
          title: "软件更新", kind: "info", okLabel: "安装", cancelLabel: "稍后",
        })
        if (confirmed) {
          try { await tauriApi.installAppUpdate() } catch { await savePackage() }
        }
      } catch { await savePackage() }
    }

    const terminal = useTerminalStore()
    if (terminal.ota.active) {
      watch(() => terminal.ota.active, () => { void offerInstall() }, { once: true })
    } else {
      await offerInstall()
    }
  } catch {
    // Network, server and download failures never interrupt normal app use.
  }
}
