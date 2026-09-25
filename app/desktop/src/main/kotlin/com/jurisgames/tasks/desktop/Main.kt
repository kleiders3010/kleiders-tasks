package com.jurisgames.tasks.desktop

import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import com.jurisgames.tasks.core.KtNode
import com.jurisgames.tasks.core.NodeOptions
import com.jurisgames.tasks.core.Platform
import com.jurisgames.tasks.ui.TasksApp
import java.io.File

/**
 * Desktop app (Windows, Linux). Data lives in the platform's app-data folder; the identity
 * next to it but outside anything backed up. Paths can be overridden for tests:
 * `-Dkt.home=DIR -Dkt.worker=PATH -Dkt.p2p=false -Dkt.relays=ws://...`.
 */
fun openNode(): KtNode {
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
    return KtNode.open(identity.absolutePath, data.absolutePath, worker, NodeOptions(Platform.DESKTOP, relays, p2p, p2p, bind))
}

private fun isWindows() = System.getProperty("os.name").lowercase().contains("win")

fun main() {
    val node = openNode()
    node.startLive() // the desktop app stays live while it runs (tray app later)
    application {
        Window(onCloseRequest = { node.stopLive(); node.syncNow(); exitApplication() }, title = "Tasks") {
            TasksApp(node)
        }
    }
}
