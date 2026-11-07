package io.github.moorew.tailcopy.core

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.widget.Toast
import kotlinx.coroutines.launch

/**
 * Saves into TailCopy without opening the app (SPEC 21.4): "Save to TailCopy" in the
 * text-selection menu, the share sheet, and the Quick Settings tile. The tile case
 * waits for window focus, because only a focused app may read the clipboard.
 */
class SaveActivity : Activity() {
  companion object {
    const val ACTION_SAVE_CLIPBOARD = "io.github.moorew.tailcopy.SAVE_CLIPBOARD"
  }

  private var done = false

  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    when (intent?.action) {
      Intent.ACTION_PROCESS_TEXT -> {
        val text = intent.getCharSequenceExtra(Intent.EXTRA_PROCESS_TEXT)?.toString()
        save { CoreHolder.saveShared(this, text, null, callingPackage) }
      }
      Intent.ACTION_SEND -> {
        val text = intent.getStringExtra(Intent.EXTRA_TEXT)
        val uri = streamUri(intent)
        save {
          val image = uri?.let { u -> runCatching { contentResolver.openInputStream(u)?.use { it.readBytes() } }.getOrNull() }
          CoreHolder.saveShared(this, text, image, referrer?.host)
        }
      }
      ACTION_SAVE_CLIPBOARD -> {} // onWindowFocusChanged
      else -> finish()
    }
  }

  override fun onWindowFocusChanged(hasFocus: Boolean) {
    super.onWindowFocusChanged(hasFocus)
    if (hasFocus && !done && intent?.action == ACTION_SAVE_CLIPBOARD) {
      done = true
      CoreHolder.scope.launch {
        val result = CoreHolder.captureClipboard(this@SaveActivity)
        if (result == "saved") CatchUpWorker.soon(applicationContext)
        runOnUiThread {
          toast(
            when (result) {
              "saved" -> R.string.tc_saved
              "sensitive" -> R.string.tc_sensitive
              else -> R.string.tc_nothing
            },
          )
          finish()
        }
      }
    }
  }

  private fun save(block: suspend () -> Boolean) {
    CoreHolder.scope.launch {
      val saved = runCatching { block() }.getOrDefault(false)
      if (saved) CatchUpWorker.soon(applicationContext)
      runOnUiThread {
        toast(if (saved) R.string.tc_saved else R.string.tc_nothing)
        finish()
      }
    }
  }

  private fun toast(text: Int) = Toast.makeText(applicationContext, text, Toast.LENGTH_SHORT).show()

  private fun streamUri(intent: Intent): Uri? =
    if (Build.VERSION.SDK_INT >= 33) {
      intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
    } else {
      @Suppress("DEPRECATION")
      intent.getParcelableExtra(Intent.EXTRA_STREAM)
    }
}
