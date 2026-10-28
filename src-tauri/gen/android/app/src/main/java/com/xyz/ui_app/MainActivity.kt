package com.xyz.ui_app

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.View
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.annotation.Keep
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.core.content.FileProvider
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import java.io.File

class MainActivity : TauriActivity() {
  @Keep
  fun installAppUpdate(path: String): Boolean = try {
    val file = File(path).canonicalFile
    require(file.isFile && file.extension.equals("apk", ignoreCase = true))
    require(file.parentFile == File(cacheDir, "app-update").canonicalFile)
    val uri = FileProvider.getUriForFile(this, "$packageName.fileprovider", file)
    startActivity(Intent(Intent.ACTION_VIEW).apply {
      setDataAndType(uri, "application/vnd.android.package-archive")
      addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    })
    true
  } catch (error: Exception) {
    Log.w("RideX", "Failed to open app installer", error)
    false
  }

  @Keep
  fun setOtaBackground(active: Boolean): Boolean = try {
    val intent = Intent(this, OtaService::class.java)
    if (active) ContextCompat.startForegroundService(this, intent) else stopService(intent)
    true
  } catch (error: Exception) {
    Log.e("RideX", "Failed to update OTA background service", error)
    false
  }

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    webView.settings.apply {
      setSupportZoom(false)
      builtInZoomControls = false
      displayZoomControls = false
    }
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    requestBlePermissions()

    val rootView = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(rootView) { v, insets ->
      val safeDrawing = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
      )
      val imeVisible = insets.isVisible(WindowInsetsCompat.Type.ime())
      val imeHeight = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
      val bottomPadding = if (imeVisible) imeHeight else safeDrawing.bottom
      v.setPadding(safeDrawing.left, safeDrawing.top, safeDrawing.right, bottomPadding)
      insets
    }
  }

  private fun requestBlePermissions() {
    val permissions = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
      arrayOf(Manifest.permission.BLUETOOTH_SCAN, Manifest.permission.BLUETOOTH_CONNECT)
    } else {
      arrayOf(Manifest.permission.ACCESS_FINE_LOCATION)
    }
    val missing = permissions.filter {
      ActivityCompat.checkSelfPermission(this, it) != PackageManager.PERMISSION_GRANTED
    }
    if (missing.isNotEmpty()) ActivityCompat.requestPermissions(this, missing.toTypedArray(), 1001)
  }
}
