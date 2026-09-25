package com.jurisgames.tasks

import android.content.Context
import androidx.work.CoroutineWorker
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.OutOfQuotaPolicy
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import androidx.work.ExistingWorkPolicy

/** One sync round on every layer (periodic, and expedited when leaving the app). */
class SyncWorker(ctx: Context, params: WorkerParameters) : CoroutineWorker(ctx, params) {
    override suspend fun doWork(): Result {
        (applicationContext as TasksApplication).node.syncNow()
        return Result.success()
    }

    companion object {
        /** Leaving the app: send pending changes now (expedited, no notification; DESIGN §8). */
        fun flushNow(ctx: Context) {
            val req = OneTimeWorkRequestBuilder<SyncWorker>().setExpedited(OutOfQuotaPolicy.RUN_AS_NON_EXPEDITED_WORK_REQUEST).build()
            WorkManager.getInstance(ctx).enqueueUniqueWork("flush", ExistingWorkPolicy.REPLACE, req)
        }
    }
}
