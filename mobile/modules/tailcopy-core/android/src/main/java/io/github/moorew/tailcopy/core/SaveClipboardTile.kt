package io.github.moorew.tailcopy.core

import android.app.PendingIntent
import android.content.Intent
import android.os.Build
import android.service.quicksettings.TileService

/** Quick Settings "Save clipboard" (SPEC 21.4): opens [SaveActivity], which has focus. */
class SaveClipboardTile : TileService() {
  override fun onClick() {
    super.onClick()
    val intent = Intent(this, SaveActivity::class.java)
      .setAction(SaveActivity.ACTION_SAVE_CLIPBOARD)
      .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    if (Build.VERSION.SDK_INT >= 34) {
      val pending = PendingIntent.getActivity(this, 0, intent, PendingIntent.FLAG_IMMUTABLE)
      startActivityAndCollapse(pending)
    } else {
      @Suppress("DEPRECATION", "StartActivityAndCollapseDeprecated")
      startActivityAndCollapse(intent)
    }
  }
}
