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

    /** Registers the app Context with the Rust side (src/android.rs in kt-ffi). */
    private external fun initAndroidContext(context: android.content.Context)

    private fun open(bytes: ByteArray?): KtNode {
        // Same library JNA uses; loading it this way lets it see the app Context, which
        // iroh needs for the system DNS servers and network changes.
        System.loadLibrary("kt_ffi")
        initAndroidContext(this)
        val identity = File(noBackupFilesDir, "identity") // never backed up (D1)
        val data = File(filesDir, "data") // not backed up either: rebuilt from the backup file
        identity.mkdirs()
        data.mkdirs()
        // The import sandbox worker ships as a native "library" so Android lets us run it.
        val worker = File(applicationInfo.nativeLibraryDir, "libkt_import.so").absolutePath
        // The only thing Android backup takes (res/xml/*backup*): restored on a new phone
        // before the first launch, and then picked up by KtNode.open automatically.
        val backup = File(filesDir, "backup").absolutePath
        val options = NodeOptions(Platform.PHONE, emptyList(), true, true, "", android.os.Build.MODEL, backup)
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

    /** Wi-Fi and charging decide whether this phone helps others sync right now (§7a). */
    fun reportDeviceState() {
        val cm = getSystemService(android.net.ConnectivityManager::class.java)
        val wifi = cm.getNetworkCapabilities(cm.activeNetwork)?.let {
            it.hasTransport(android.net.NetworkCapabilities.TRANSPORT_WIFI) || it.hasTransport(android.net.NetworkCapabilities.TRANSPORT_ETHERNET)
        } ?: false
        val charging = getSystemService(android.os.BatteryManager::class.java).isCharging
        node.setDeviceState(wifi, charging)
    }

    /** "Sync in the background": periodic work on or off. */
    fun applyBackgroundSync(on: Boolean) {
        if (on) schedulePeriodic() else WorkManager.getInstance(this).cancelUniqueWork("sync")
    }

    override fun onCreate() {
        super.onCreate()
        schedulePeriodic() // the worker itself checks "Sync in the background"
    }

    private fun schedulePeriodic() {
        // Background sync: WorkManager's 15-minute minimum; Doze stretches it (DESIGN §8).
        val periodic = PeriodicWorkRequestBuilder<SyncWorker>(15, TimeUnit.MINUTES)
            .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
            .build()
        WorkManager.getInstance(this).enqueueUniquePeriodicWork("sync", ExistingPeriodicWorkPolicy.KEEP, periodic)
    }
}
