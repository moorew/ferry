package io.github.moorew.tailcopy.core

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Build
import android.os.PersistableBundle
import android.provider.Settings
import androidx.core.content.FileProvider
import java.io.File
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import uniffi.tailcopy_mobile.ClipInput
import uniffi.tailcopy_mobile.CoreListener
import uniffi.tailcopy_mobile.TailCopy

/** The one TailCopy core in the app process, and the clipboard around it (SPEC 21.2). */
object CoreHolder : CoreListener {
  private const val OWN_LABEL = "TailCopy"
  // ClipDescription.EXTRA_IS_SENSITIVE (API 33) by value, so older releases match too.
  private const val EXTRA_IS_SENSITIVE = "android.content.extra.IS_SENSITIVE"
  private const val EXTRA_IS_REMOTE_DEVICE = "android.content.extra.IS_REMOTE_DEVICE"

  private var instance: TailCopy? = null
  private lateinit var app: Context
  val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

  /** Set by the module; called on the main thread. */
  @Volatile var onChanged: (() -> Unit)? = null
  @Volatile private var liveMirror = false
  /** Timestamp of the last clip TailCopy read or wrote. */
  @Volatile private var seenStamp = -1L
  /** TailCopy surfaces on screen; sync runs while this is above zero. */
  private val visible = AtomicInteger(0)

  @Synchronized
  fun core(context: Context): TailCopy {
    instance?.let { return it }
    app = context.applicationContext
    val core = TailCopy.open(
      File(app.filesDir, "tailcopy").path,
      null,
      deviceName(app),
      this,
    )
    instance = core
    scope.launch { liveMirror = core.setting("live_mirror") == "on" }
    return core
  }

  private fun deviceName(context: Context): String =
    Settings.Global.getString(context.contentResolver, Settings.Global.DEVICE_NAME)
      ?.takeIf { it.isNotBlank() } ?: Build.MODEL

  fun setLiveMirror(on: Boolean) {
    liveMirror = on
  }

  /** A TailCopy screen, sheet or activity came to the front. */
  fun shown(context: Context) {
    if (visible.incrementAndGet() == 1) core(context).startSync()
  }

  fun hidden(context: Context) {
    if (visible.decrementAndGet() <= 0) {
      visible.set(0)
      core(context).stopSync()
    }
  }

  val isVisible: Boolean
    get() = visible.get() > 0

  override fun changed() {
    android.os.Handler(android.os.Looper.getMainLooper()).post { onChanged?.invoke() }
  }

  override fun liveClip(clipId: String) {
    if (!liveMirror || !isVisible) return
    scope.launch { copy(app, clipId, plainText = false, markUsed = false) }
  }

  /**
   * Reads the clipboard if it changed since TailCopy last saw it (SPEC 21.4). Only
   * works while a TailCopy window has focus. The description is read first: it shows
   * no "pasted" toast and carries the sensitive flag.
   */
  suspend fun captureClipboard(context: Context): String {
    val cm = context.getSystemService(ClipboardManager::class.java)
    val desc = cm.primaryClipDescription ?: return "empty"
    if (desc.timestamp == seenStamp) return "unchanged"
    seenStamp = desc.timestamp
    if (desc.label == OWN_LABEL) return "unchanged"
    if (desc.extras?.getBoolean(EXTRA_IS_SENSITIVE) == true) return "sensitive"
    val clip = cm.primaryClip ?: return "denied"
    if (clip.itemCount == 0) return "empty"
    val item = clip.getItemAt(0)
    var image: ByteArray? = null
    if (item.uri != null && desc.hasMimeType("image/*")) {
      image = runCatching {
        context.contentResolver.openInputStream(item.uri)?.use { it.readBytes() }
      }.getOrNull()
    }
    val input = ClipInput(
      text = item.text?.toString(),
      html = item.htmlText,
      rtf = null,
      image = image,
    )
    if (input.text.isNullOrEmpty() && input.html == null && input.image == null) return "empty"
    val saved = core(context).capture(input, null, false, false)
    return if (saved) "saved" else "empty"
  }

  /**
   * Puts a clip on the clipboard. Images go through a FileProvider URI; clips from
   * other devices are marked remote.
   */
  suspend fun copy(context: Context, clipId: String, plainText: Boolean, markUsed: Boolean): Boolean {
    val core = core(context)
    val content = core.content(clipId, 3000u)
    val cm = context.getSystemService(ClipboardManager::class.java)
    val png = content.png
    val clip = when {
      !plainText && png != null -> {
        val dir = File(context.cacheDir, "clips").apply { mkdirs() }
        dir.listFiles()?.forEach { it.delete() }
        val file = File(dir, "clip.png").apply { writeBytes(png) }
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.clips", file)
        ClipData.newUri(context.contentResolver, OWN_LABEL, uri)
      }
      !plainText && content.html != null ->
        ClipData.newHtmlText(OWN_LABEL, content.text ?: "", content.html)
      content.text != null -> ClipData.newPlainText(OWN_LABEL, content.text)
      else -> return false
    }
    if (content.remote) {
      clip.description.extras = PersistableBundle().apply { putBoolean(EXTRA_IS_REMOTE_DEVICE, true) }
    }
    cm.setPrimaryClip(clip)
    seenStamp = cm.primaryClipDescription?.timestamp ?: seenStamp
    if (markUsed) core.used(clipId)
    return true
  }

  /** Saves text handed over by the selection menu or a share (SPEC 21.4). */
  suspend fun saveShared(context: Context, text: String?, image: ByteArray?, source: String?): Boolean {
    val input = ClipInput(text = text, html = null, rtf = null, image = image)
    return core(context).capture(input, source, false, false)
  }
}
