package com.xyz.ui_app

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.view.View
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.app.ActivityCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
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
