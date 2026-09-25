package com.jurisgames.tasks

import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
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
            TasksApp(node, onRestore = { pick.launch(arrayOf("*/*")) }, onExport = { l ->
                // Markdown through the share sheet (Obsidian, notes apps, email).
                val send = Intent(Intent.ACTION_SEND).setType("text/markdown")
                    .putExtra(Intent.EXTRA_SUBJECT, l.name)
                    .putExtra(Intent.EXTRA_TEXT, node.exportMarkdown(l.id))
                startActivity(Intent.createChooser(send, "Export ${l.name}"))
            })
        }
    }

    override fun onStart() {
        super.onStart()
        app.node.startLive() // app open: live on every layer
    }

    override fun onStop() {
        app.node.stopLive()
        SyncWorker.flushNow(this) // everything is saved; send it without waiting (§8)
        super.onStop()
    }
}
