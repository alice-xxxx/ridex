package com.xyz.systemble

import android.Manifest
import android.app.Activity
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.content.pm.PackageManager
import android.os.Build
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import org.json.JSONArray
import org.json.JSONObject
import java.util.UUID

@InvokeArg
class ConnectedArgs {
    lateinit var serviceUuid: String
}

@TauriPlugin
class SystemBlePlugin(private val activity: Activity) : Plugin(activity) {
    @Command
    fun connectedDevices(invoke: Invoke) {
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S &&
                activity.checkSelfPermission(Manifest.permission.BLUETOOTH_CONNECT) != PackageManager.PERMISSION_GRANTED) {
                invoke.reject("缺少蓝牙连接权限")
                return
            }
            val service = UUID.fromString(invoke.parseArgs(ConnectedArgs::class.java).serviceUuid)
            val manager = activity.getSystemService(BluetoothManager::class.java)
                ?: throw IllegalStateException("没有可用的蓝牙适配器")
            val devices = JSONArray()
            for (device in manager.getConnectedDevices(BluetoothProfile.GATT)) {
                // Filter known UUIDs without opening a GATT connection.
                if (device.uuids?.any { it.uuid == service } != true) continue
                devices.put(JSONObject().put("address", device.address)
                    .put("name", device.name?.takeIf { it.isNotBlank() } ?: device.address))
            }
            invoke.resolve(JSObject().put("devices", devices))
        } catch (error: Exception) {
            invoke.reject(error.message ?: "查询系统 BLE 设备失败")
        }
    }
}
