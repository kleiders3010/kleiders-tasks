package com.jurisgames.tasks.desktop

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import com.jurisgames.tasks.core.KtNode
import com.jurisgames.tasks.core.NodeOptions
import com.jurisgames.tasks.core.Platform
import com.jurisgames.tasks.ui.TasksApp
import java.awt.FileDialog
import java.awt.Frame
import java.io.File

/**
 * Desktop app (Windows, Linux). Data lives in the platform's app-data folder; the identity
 * next to it but outside anything backed up. Paths can be overridden for tests:
 * `-Dkt.home=DIR -Dkt.worker=PATH -Dkt.p2p=false -Dkt.relays=ws://...`.
 * A backup file given as `backup` is restored (first launch, nothing made yet).
 */
fun openNode(backup: ByteArray? = null): KtNode {
    val base = System.getProperty("kt.home")?.let { File(it) } ?: run {
        val appData = System.getenv("APPDATA") ?: (System.getProperty("user.home") + "/.local/share")
        File(appData, "JurisTasks")
    }
    val identity = File(base, "identity").apply { mkdirs() }
    val data = File(base, "data").apply { mkdirs() }
    val worker = System.getProperty("kt.worker") ?: File(System.getProperty("kt.bin", "."), if (isWindows()) "kt-import-worker.exe" else "kt-import-worker").absolutePath
    val relays = System.getProperty("kt.relays")?.split(",")?.filter { it.isNotBlank() } ?: emptyList()
    val p2p = System.getProperty("kt.p2p", "true").toBoolean()
    val bind = System.getProperty("kt.bind", "")
    // Daily sealed backup (the folder choice prompt comes with the polished UI, DESIGN §11).
    val backups = System.getProperty("kt.backups") ?: File(base, "backups").absolutePath
    val options = NodeOptions(Platform.DESKTOP, relays, p2p, p2p, bind, backups)
    return if (backup == null) KtNode.open(identity.absolutePath, data.absolutePath, worker, options)
    else KtNode.openRestoring(identity.absolutePath, data.absolutePath, worker, options, backup)
}

/** Replace a fresh node with one restored from `backup` (falls back to the old one on failure). */
fun restoreNode(old: KtNode, backup: ByteArray): Result<KtNode> {
    old.stopLive()
    old.close()
    return runCatching { openNode(backup) }
}

private fun isWindows() = System.getProperty("os.name").lowercase().contains("win")

fun main() {
    val first = openNode()
    first.startLive() // the desktop app stays live while it runs (tray app later)
    application {
        var node by remember { mutableStateOf(first) }
        Window(onCloseRequest = { node.stopLive(); node.syncNow(); exitApplication() }, title = "Tasks") {
            TasksApp(node, onRestore = {
                val dialog = FileDialog(null as Frame?, "Restore from a backup file", FileDialog.LOAD).apply { isVisible = true }
                val file = dialog.file?.let { File(dialog.directory, it) }
                if (file != null) {
                    node = restoreNode(node, file.readBytes()).getOrElse { e ->
                        System.err.println("restore failed: ${e.message}") // prototype: no error UI yet
                        openNode()
                    }
                    node.startLive()
                }
            }, onExport = { l ->
                // Both formats side by side: readable Markdown and the re-importable file.
                val dialog = FileDialog(null as Frame?, "Export ${l.name}", FileDialog.SAVE).apply { file = "${l.name}.md"; isVisible = true }
                val md = dialog.file?.let { File(dialog.directory, it) }
                if (md != null) {
                    md.writeText(node.exportMarkdown(l.id))
                    File(md.parentFile, md.nameWithoutExtension + ".tasklist").writeBytes(node.exportListFile(l.id))
                }
            })
        }
    }
}
