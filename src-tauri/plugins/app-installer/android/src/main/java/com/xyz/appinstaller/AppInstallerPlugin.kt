package com.xyz.appinstaller

import android.app.Activity
import android.content.Intent
import androidx.core.content.FileProvider
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin
import java.io.File

class AppInstallerProvider : FileProvider()

@InvokeArg
class InstallArgs {
    lateinit var path: String
}

@TauriPlugin
class AppInstallerPlugin(private val activity: Activity) : Plugin(activity) {
    @Command
    fun install(invoke: Invoke) {
        try {
            val file = File(invoke.parseArgs(InstallArgs::class.java).path).canonicalFile
            require(file.isFile && file.extension.equals("apk", ignoreCase = true))
            require(file.parentFile == File(activity.cacheDir, "app-update").canonicalFile)
            val uri = FileProvider.getUriForFile(activity, "${activity.packageName}.appinstaller.fileprovider", file)
            activity.startActivity(Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, "application/vnd.android.package-archive")
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            })
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject("请允许安装未知应用；也可以保存安装包后手动安装", error)
        }
    }
}
