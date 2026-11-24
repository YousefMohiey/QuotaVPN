package com.quotacards.app

import android.graphics.Color
import android.os.Bundle
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.activity.SystemBarStyle
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import com.quotacards.tunnel.TunnelService

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    // The app is dark in every system state: force light system-bar icons
    // rather than letting a light phone theme paint dark icons on the night
    // field. The content root is then padded by the status-bar height.
    enableEdgeToEdge(
      statusBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
      navigationBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
    )
    super.onCreate(savedInstanceState)
    // Edge-to-edge draws the WebView under the status bar - push the content
    // below it so the app header never sits under the clock/battery icons.
    val root = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(root) { v, insets ->
      val bars = insets.getInsets(WindowInsetsCompat.Type.statusBars())
      v.setPadding(0, bars.top, 0, 0)
      insets
    }
    // System back walks in-app history (tabs) first. At the last page:
    // VPN on = background the app (HOME behavior), never finish - finishing
    // the activity is what takes the tunnel down on some phones.
    // VPN off = close the UI as normal.
    onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
      override fun handleOnBackPressed() {
        val wv = findWebView(findViewById(android.R.id.content))
        if (wv != null && wv.canGoBack()) {
          wv.goBack()
        } else if (TunnelService.running) {
          this@MainActivity.moveTaskToBack(true)
        } else {
          isEnabled = false
          this@MainActivity.onBackPressedDispatcher.onBackPressed()
          isEnabled = true
        }
      }
    })
  }

  private fun findWebView(v: View?): WebView? {
    if (v == null) return null
    if (v is WebView) return v
    if (v is ViewGroup) {
      for (i in 0 until v.childCount) {
        findWebView(v.getChildAt(i))?.let { return it }
      }
    }
    return null
  }
}
