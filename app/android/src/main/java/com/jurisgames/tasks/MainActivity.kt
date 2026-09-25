package com.jurisgames.tasks

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.lifecycle.lifecycleScope
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import com.jurisgames.tasks.ui.TasksApp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class MainActivity : ComponentActivity() {
    private val app get() = application as TasksApplication

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        opened.value = intent?.dataString
        setContent {
            var node by remember { mutableStateOf(app.node) }
            val scope = rememberCoroutineScope()
            val pick = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
                if (uri != null) scope.launch {
                    try {
                        node = withContext(Dispatchers.IO) {
                            val bytes = contentResolver.openInputStream(uri)!!.use { it.readBytes() }
                            app.restoreFrom(bytes)
                        }
                        node.startLive()
                    } catch (e: Exception) {
                        node = app.node
                        Toast.makeText(this@MainActivity, e.message ?: "Restore failed", Toast.LENGTH_LONG).show()
                    }
                }
            }
            TasksApp(node, onShare = { link ->
                startActivity(Intent.createChooser(Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, link), "Share invite"))
            }, update = update.value, onUpdate = { link ->
                startActivity(Intent(Intent.ACTION_VIEW, android.net.Uri.parse(link)))
            }, onScan = { QrScanner.scan(this, it) }, openedCode = opened.value, onSettings = { app.applyBackgroundSync(it.syncInBackground) }, onRestore = { pick.launch(arrayOf("*/*")) }, onExport = { l ->
                // Markdown through the share sheet (Obsidian, notes apps, email).
                val send = Intent(Intent.ACTION_SEND).setType("text/markdown")
                    .putExtra(Intent.EXTRA_SUBJECT, l.name)
                    .putExtra(Intent.EXTRA_TEXT, node.exportMarkdown(l.id))
                startActivity(Intent.createChooser(send, "Export ${l.name}"))
            })
        }
    }

    /** A newer version, if the daily check found one. */
    private val update = mutableStateOf<com.jurisgames.tasks.core.UpdateItem?>(null)

    /** At most once a day, off the main thread. */
    private fun checkForUpdate() {
        val prefs = getSharedPreferences("update", MODE_PRIVATE)
        val now = System.currentTimeMillis()
        if (now - prefs.getLong("checked", 0) < 24 * 3600_000L) return
        val current = packageManager.getPackageInfo(packageName, 0).longVersionCode.toULong()
        lifecycleScope.launch(Dispatchers.IO) {
            val found = app.node.checkUpdate(current, Store.UPDATE_CHANNEL)
            prefs.edit().putLong("checked", now).apply()
            update.value = found
        }
    }

    /** The invite or device link that opened the app, if any. */
    private val opened = mutableStateOf<String?>(null)

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        opened.value = intent.dataString
    }

    override fun onStart() {
        super.onStart()
        app.node.markSeen() // what's on screen needs no notification
        app.reportDeviceState()
        app.node.startLive() // app open: live on every layer
        askForNotificationsWhenShared()
        checkForUpdate()
    }

    /** Asked the first time it's relevant: once this device is in a shared group (§12). */
    private val notifyPermission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { }

    private fun askForNotificationsWhenShared() {
        if (Build.VERSION.SDK_INT < 33 || checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED) return
        if (app.node.groups().any { !it.personal }) notifyPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
    }

    override fun onStop() {
        lifecycleScope.launch { TasksWidget.refresh(applicationContext) } // edits made in the app
        app.node.stopLive()
        SyncWorker.flushNow(this) // everything is saved; send it without waiting (§8)
        super.onStop()
    }
}
