package com.jurisgames.tasks.desktop

import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import com.jurisgames.tasks.core.KtNode
import com.jurisgames.tasks.core.NodeOptions
import com.jurisgames.tasks.core.Platform
import com.jurisgames.tasks.ui.TasksApp
import org.junit.Rule
import org.junit.Test
import java.io.File
import java.nio.file.Files

/** Headless desktop UI test (offscreen): Compose UI → JNA/UniFFI → the Rust library. */
class DesktopSmokeTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun createAListAndATask() {
        val home = Files.createTempDirectory("kt-desktop").toFile()
        val worker = System.getProperty("kt.worker")
        // No network: a dead relay and no P2P (nothing leaves this machine).
        val node = KtNode.open(File(home, "identity").path, File(home, "data").path, worker, NodeOptions(Platform.DESKTOP, listOf("ws://127.0.0.1:9"), false, false, "127.0.0.1:0", "", ""))
        rule.setContent { TasksApp(node) }
        rule.onNodeWithTag("newgroup").performTextInput("Family")
        rule.onNodeWithTag("addgroup").performClick()
        rule.waitUntil(5_000) { rule.onAllNodes(hasTestTag("newlist-Family")).fetchSemanticsNodes().isNotEmpty() }
        rule.onNodeWithTag("newlist-Family").performTextInput("Groceries")
        rule.onNodeWithTag("addlist-Family").performClick()
        rule.waitUntil(5_000) { rule.onAllNodes(hasTestTag("list-Groceries")).fetchSemanticsNodes().isNotEmpty() }
        rule.onNodeWithTag("list-Groceries").performClick()
        rule.onNodeWithTag("newtask").performTextInput("milk")
        rule.onNodeWithTag("addtask").performClick()
        rule.waitUntil(5_000) { rule.onAllNodes(hasTestTag("task-milk")).fetchSemanticsNodes().isNotEmpty() }
        rule.onNodeWithTag("done-milk").performClick()
        check(node.tasks(node.lists().first { it.name == "Groceries" }.id).single().done) { "done state reached the engine" }
        // An invite from the desktop app: 8 words.
        val family = node.groups().first { it.name == "Family" }
        val invite = node.invite(family.id, 30u)
        check(invite.words.size == 8)
        check(invite.link.startsWith("https://jurisgames.com/join#" + invite.words.joinToString("-"))) { invite.link }
        check(com.jurisgames.tasks.core.qrMatrix(invite.link).size >= 21u)
    }
}
