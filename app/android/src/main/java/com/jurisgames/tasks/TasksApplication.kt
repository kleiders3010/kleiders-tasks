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
    private var current: KtNode? = null

    /** One node per process; opened on first use (UI or background job). */
    val node: KtNode
        @Synchronized get() = current ?: open(null).also { current = it }

    private fun open(bytes: ByteArray?): KtNode {
        val identity = File(noBackupFilesDir, "identity") // never backed up (D1)
        val data = File(filesDir, "data") // not backed up either: rebuilt from the backup file
        identity.mkdirs()
        data.mkdirs()
        // The import sandbox worker ships as a native "library" so Android lets us run it.
        val worker = File(applicationInfo.nativeLibraryDir, "libkt_import.so").absolutePath
        // The only thing Android backup takes (res/xml/*backup*): restored on a new phone
        // before the first launch, and then picked up by KtNode.open automatically.
        val backup = File(filesDir, "backup").absolutePath
        val options = NodeOptions(Platform.PHONE, emptyList(), true, true, "", backup)
        return if (bytes == null) KtNode.open(identity.absolutePath, data.absolutePath, worker, options)
        else KtNode.openRestoring(identity.absolutePath, data.absolutePath, worker, options, bytes)
    }

    /** First launch, restore skipped during phone setup: restore a backup file the user picked. */
    @Synchronized
    fun restoreFrom(bytes: ByteArray): KtNode {
        current?.let { it.stopLive(); it.close() }
        current = null
        return open(bytes).also { current = it }
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
