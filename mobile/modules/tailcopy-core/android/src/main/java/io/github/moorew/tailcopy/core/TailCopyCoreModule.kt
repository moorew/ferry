package io.github.moorew.tailcopy.core

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.core.os.bundleOf
import expo.modules.kotlin.functions.Coroutine
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import uniffi.tailcopy_mobile.ClipInput
import uniffi.tailcopy_mobile.ClipItem
import uniffi.tailcopy_mobile.DeviceItem

private fun ClipItem.toBundle(): Bundle = bundleOf(
  "id" to id, "kind" to kind, "look" to look, "preview" to preview, "label" to label,
  "pinned" to pinned, "mono" to mono, "deviceId" to deviceId, "deviceName" to deviceName,
  "colorIndex" to colorIndex.toInt(), "fromThisDevice" to fromThisDevice,
  "createdMs" to createdMs.toDouble(), "byteSize" to byteSize.toDouble(), "localOnly" to localOnly,
  "thumbnail" to thumbnail?.let { "file://$it" },
)

private fun DeviceItem.toBundle(): Bundle = bundleOf(
  "id" to id, "name" to name, "role" to role, "colorIndex" to colorIndex.toInt(),
  "isSelf" to isSelf, "trusted" to trusted, "connected" to connected, "caughtUp" to caughtUp,
  "lastSyncMs" to lastSyncMs?.toDouble(), "error" to error, "send" to send, "receive" to receive,
)

class TailCopyCoreModule : Module() {
  private val context get() = requireNotNull(appContext.reactContext)
  private val core get() = CoreHolder.core(context)

  override fun definition() = ModuleDefinition {
    Name("TailCopyCore")

    Events("changed")

    OnCreate {
      CoreHolder.onChanged = { sendEvent("changed") }
      CatchUpWorker.schedule(context)
    }

    OnActivityEntersForeground { CoreHolder.shown(context) }
    OnActivityEntersBackground { CoreHolder.hidden(context) }

    AsyncFunction("captureClipboard") Coroutine { -> CoreHolder.captureClipboard(context) }

    AsyncFunction("captureText") Coroutine { text: String ->
      core.capture(ClipInput(text, null, null, null), null, false, false)
    }

    AsyncFunction("list") Coroutine { query: String, offset: Int, limit: Int, today: Double, yesterday: Double ->
      core.list(query, offset.toUInt(), limit.toUInt(), today.toULong(), yesterday.toULong()).map { it.toBundle() }
    }

    AsyncFunction("detail") Coroutine { id: String ->
      val c = core.content(id, 3000u)
      bundleOf("text" to c.text, "hasImage" to (c.png != null), "incomplete" to c.incomplete)
    }

    AsyncFunction("copy") Coroutine { id: String, plainText: Boolean ->
      CoreHolder.copy(context, id, plainText, markUsed = true)
    }

    AsyncFunction("setPinned") Coroutine { id: String, pinned: Boolean -> core.setPinned(id, pinned) }
    AsyncFunction("setLabel") Coroutine { id: String, label: String? -> core.setLabel(id, label) }
    AsyncFunction("remove") Coroutine { id: String -> core.delete(id) }
    AsyncFunction("clearHistory") Coroutine { includePinned: Boolean -> core.clearHistory(includePinned) }
    AsyncFunction("devices") Coroutine { -> core.devices().map { it.toBundle() } }
    AsyncFunction("setDeviceSync") Coroutine { id: String, send: Boolean, receive: Boolean ->
      core.setDeviceSync(id, send, receive)
    }
    AsyncFunction("renameThisDevice") Coroutine { name: String -> core.renameThisDevice(name) }
    AsyncFunction("pair") Coroutine { link: String ->
      val p = core.pair(link)
      bundleOf("deviceId" to p.deviceId, "name" to p.name)
    }
    AsyncFunction("forget") Coroutine { id: String -> core.forget(id) }
    AsyncFunction("forgetAll") Coroutine { -> core.forgetAll() }

    Function("status") {
      val s = core.status()
      bundleOf("state" to s.state, "connected" to s.connected.toInt())
    }
    Function("tailnetConnected") { core.tailnetConnected() }
    Function("retry") { core.retry() }

    AsyncFunction("setting") Coroutine { key: String -> core.setting(key) }
    AsyncFunction("setSetting") Coroutine { key: String, value: String ->
      core.setSetting(key, value)
      if (key == "live_mirror") CoreHolder.setLiveMirror(value == "on")
    }

    AsyncFunction("openTailscale") {
      val launch = context.packageManager.getLaunchIntentForPackage("com.tailscale.ipn")
        ?: Intent(Intent.ACTION_VIEW, Uri.parse("market://details?id=com.tailscale.ipn"))
      launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
      runCatching { context.startActivity(launch) }.isSuccess
    }
  }
}
