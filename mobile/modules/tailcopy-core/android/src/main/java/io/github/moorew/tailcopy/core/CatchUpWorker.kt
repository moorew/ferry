package io.github.moorew.tailcopy.core

import android.content.Context
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.delay

/** A periodic catch-up while the app isn't on screen (SPEC 21.3.6). */
class CatchUpWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
  override suspend fun doWork(): Result {
    val core = CoreHolder.core(applicationContext)
    if (CoreHolder.isVisible) return Result.success()
    core.startSync()
    delay(25_000)
    if (!CoreHolder.isVisible) core.stopSync()
    return Result.success()
  }

  companion object {
    fun schedule(context: Context) {
      val request = PeriodicWorkRequestBuilder<CatchUpWorker>(15, TimeUnit.MINUTES)
        .setConstraints(
          Constraints.Builder().setRequiredNetworkType(NetworkType.UNMETERED).build(),
        )
        .build()
      WorkManager.getInstance(context)
        .enqueueUniquePeriodicWork("tailcopy-catch-up", ExistingPeriodicWorkPolicy.KEEP, request)
    }

    /** Sends a clip saved from the selection menu, a share or the tile right away. */
    fun soon(context: Context) {
      val request = OneTimeWorkRequestBuilder<CatchUpWorker>()
        .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
        .build()
      WorkManager.getInstance(context)
        .enqueueUniqueWork("tailcopy-send-now", ExistingWorkPolicy.REPLACE, request)
    }
  }
}
