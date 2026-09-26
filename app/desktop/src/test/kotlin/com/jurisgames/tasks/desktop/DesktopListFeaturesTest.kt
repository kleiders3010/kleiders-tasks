package com.jurisgames.tasks.desktop

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performImeAction
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTouchInput
import com.jurisgames.tasks.core.KtNode
import com.jurisgames.tasks.core.NodeOptions
import com.jurisgames.tasks.core.Platform
import com.jurisgames.tasks.ui.TasksApp
import org.junit.Rule
import org.junit.Test
import java.awt.image.BufferedImage
import java.io.File
import java.nio.file.Files
import javax.imageio.ImageIO

/** Renaming, shopping lists, dragging to reorder and images, through the UI. */
class DesktopListFeaturesTest {
    @get:Rule
    val rule = createComposeRule()

    private fun shown(tag: String) = rule.onAllNodes(hasTestTag(tag), useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty()
    private fun top(tag: String) = rule.onNodeWithTag(tag, useUnmergedTree = true).fetchSemanticsNode().boundsInRoot.top

    private fun node(): KtNode {
        val home = Files.createTempDirectory("kt-feat").toFile()
        val options = NodeOptions(Platform.DESKTOP, listOf("ws://127.0.0.1:9"), false, false, "127.0.0.1:0", "", "")
        return KtNode.open(File(home, "identity").path, File(home, "data").path, System.getProperty("kt.worker"), options)
    }

    @Test
    fun renameShoppingDragAndImages() {
        val n = node()
        val list = n.createList(null, "Groceries")
        for (t in listOf("milk", "eggs", "bread")) n.addTask(list.id, t, null)
        val decode = Images::decode
        rule.setContent { TasksApp(n, decodeImage = decode) }
        rule.onNodeWithTag("list-Groceries").performClick()
        rule.waitUntil(5_000) { shown("task-milk") }

        // Tap a title to rename it.
        rule.onNodeWithTag("task-eggs", useUnmergedTree = true).performClick()
        rule.onNodeWithTag("edit-eggs").performTextClearance()
        rule.onNodeWithTag("edit-eggs").performTextInput("free-range eggs")
        rule.onNodeWithTag("edit-eggs").performImeAction()
        rule.waitUntil(5_000) { shown("task-free-range eggs") }
        check(n.tasks(list.id).map { it.title } == listOf("milk", "free-range eggs", "bread"))

        // A shopping list: a checked item stays a moment, then drops to the bottom.
        rule.onNodeWithTag("shoppingSwitch").performClick()
        rule.waitUntil(5_000) { n.lists().single().listType == "shopping" }
        rule.onNodeWithTag("done-milk", useUnmergedTree = true).performClick()
        rule.waitForIdle()
        check(top("task-milk") < top("task-bread")) { "not yet: it settles first" }
        rule.mainClock.advanceTimeBy(1_500)
        rule.waitUntil(5_000) { top("task-milk") > top("task-bread") }
        check(n.tasks(list.id).map { it.title }.first() == "milk") { "only the view moved it, not the list's order" }
        // Unchecking puts it back in its place.
        rule.onNodeWithTag("done-milk", useUnmergedTree = true).performClick()
        rule.waitUntil(5_000) { top("task-milk") < top("task-free-range eggs") }

        // Drag "bread" to the top by its handle.
        val step = top("task-free-range eggs") - top("task-milk")
        rule.onNodeWithTag("drag-bread", useUnmergedTree = true).performTouchInput {
            down(center)
            repeat(20) { moveBy(Offset(0f, -(step * 2.5f) / 20)) }
            up()
        }
        rule.waitUntil(5_000) { n.tasks(list.id).map { it.title }.first() == "bread" }

        // An image: shown as a thumbnail, full size on tap, removable.
        val png = File.createTempFile("kt-img", ".png").apply {
            val img = BufferedImage(1200, 800, BufferedImage.TYPE_INT_ARGB)
            val g = img.createGraphics(); g.color = java.awt.Color.ORANGE; g.fillRect(0, 0, 1200, 800); g.dispose()
            ImageIO.write(img, "png", this)
        }
        val bytes = Images.compress(png)!!
        val milk = n.tasks(list.id).single { it.title == "milk" }
        n.addImage(list.id, milk.id, bytes)
        rule.onNodeWithTag("styleOpen").performClick() // anything that recomposes; the node notifies too
        rule.onNodeWithTag("styleOpen").performClick()
        rule.waitUntil(5_000) { shown("thumb-milk") }
        rule.onNodeWithTag("thumb-milk", useUnmergedTree = true).performClick()
        rule.waitUntil(5_000) { shown("imageFull") }
        rule.onNodeWithTag("imageRemove").performClick()
        rule.waitUntil(5_000) { n.tasks(list.id).single { it.title == "milk" }.images.isEmpty() }
        n.close()
    }
}
