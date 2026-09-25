package com.jurisgames.tasks

import android.app.Application
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import com.jurisgames.tasks.core.KtNode
import com.jurisgames.tasks.core.NodeOptions
import com.jurisgames.tasks.core.Platform
import java.io.File
import java.util.concurrent.TimeUnit

class TasksApplication : Application() {
    /** One node per process; opened on first use (UI or background job). */
    val node: KtNode by lazy {
        val identity = File(noBackupFilesDir, "identity") // never backed up (D1)
        val data = File(filesDir, "data")
        identity.mkdirs()
        data.mkdirs()
        // The import sandbox worker ships as a native "library" so Android lets us run it.
        val worker = File(applicationInfo.nativeLibraryDir, "libkt_import.so").absolutePath
        KtNode.open(identity.absolutePath, data.absolutePath, worker, NodeOptions(Platform.PHONE, emptyList(), true, true, ""))
    }

    override fun onCreate() {
        super.onCreate()
        // Background sync: WorkManager's 15-minute minimum; Doze stretches it (DESIGN §8).
        val periodic = PeriodicWorkRequestBuilder<SyncWorker>(15, TimeUnit.MINUTES)
            .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
            .build()
        WorkManager.getInstance(this).enqueueUniquePeriodicWork("sync", ExistingPeriodicWorkPolicy.KEEP, periodic)
    }
}
