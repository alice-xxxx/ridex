package com.xyz.mobileui

import android.app.Activity
import android.view.View
import android.view.WindowManager
import android.webkit.WebView
import androidx.activity.ComponentActivity
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Plugin

@TauriPlugin
class MobileUiPlugin(private val activity: Activity) : Plugin(activity) {
    override fun load(webView: WebView) {
        (activity as ComponentActivity).enableEdgeToEdge()
        activity.window.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_NOTHING)
        webView.settings.apply {
            setSupportZoom(false)
            builtInZoomControls = false
            displayZoomControls = false
        }
        val root = activity.findViewById<View>(android.R.id.content)
        ViewCompat.setOnApplyWindowInsetsListener(root) { view, insets ->
            val safeDrawing = insets.getInsets(
                WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
            )
            view.setPadding(safeDrawing.left, safeDrawing.top, safeDrawing.right, safeDrawing.bottom)
            insets
        }
        ViewCompat.requestApplyInsets(root)
    }
}
