package com.jurisgames.tasks.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import com.jurisgames.tasks.core.KtNode
import com.jurisgames.tasks.core.TaskItem
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import sh.calvin.reorderable.ReorderableItem
import sh.calvin.reorderable.rememberReorderableLazyListState

/** Turns image bytes into something to draw (each platform provides its decoder). */
val LocalImageDecoder = staticCompositionLocalOf<((ByteArray) -> ImageBitmap?)?> { null }

/** How long a checked item stays in place in a shopping list before dropping to the bottom. */
const val SETTLE_MS = 1000L

/**
 * The order rows are shown in. Tasks keep the list's order (subtasks under their parent);
 * in a shopping list, checked items go after the unchecked ones at each level, except those
 * checked in the last moment (`settling`), which stay put until [SETTLE_MS] has passed.
 */
fun displayOrder(tasks: List<TaskItem>, shopping: Boolean, settling: Set<String>): List<TaskItem> {
    val byParent = tasks.groupBy { it.parentId }
    fun level(parent: String?): List<TaskItem> {
        val siblings = byParent[parent].orEmpty()
        val ordered = if (!shopping) siblings else siblings.sortedBy { if (it.done && it.id !in settling) 1 else 0 } // stable
        return ordered.flatMap { listOf(it) + level(it.id) }
    }
    return level(null)
}

/**
 * The list's rows: check, tap the title to rename, drag the handle to reorder (among tasks
 * at the same level), notes, images, reminder, subtask, delete. Placeholder look (UX pending).
 */
@Composable
fun TaskRows(
    node: KtNode,
    listId: String,
    tasks: List<TaskItem>,
    shopping: Boolean,
    version: Int,
    onPickImage: ((onPicked: (ByteArray) -> Unit) -> Unit)?,
    onNotes: (String) -> Unit,
    onChanged: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val settling = remember { mutableStateListOf<String>() }
    var editing by remember { mutableStateOf<String?>(null) }
    var viewing by remember { mutableStateOf<Pair<String, String>?>(null) } // task, image
    // What's shown; dragging reorders it locally until the drop is saved.
    var display by remember { mutableStateOf(displayOrder(tasks, shopping, settling.toSet())) }
    LaunchedEffect(tasks, shopping, settling.toList()) { display = displayOrder(tasks, shopping, settling.toSet()) }

    viewing?.let { (task, image) -> ImageViewer(node, listId, task, image, version, onChanged, onClose = { viewing = null }) }

    val lazy = rememberLazyListState()
    val reorder = rememberReorderableLazyListState(lazy) { from, to ->
        val a = display.getOrNull(from.index) ?: return@rememberReorderableLazyListState
        val b = display.getOrNull(to.index) ?: return@rememberReorderableLazyListState
        if (a.parentId == b.parentId) display = display.toMutableList().apply { add(to.index, removeAt(from.index)) }
    }
    // A drop: place the task next to its new neighbour at the same level.
    fun drop(id: String) {
        val i = display.indexOfFirst { it.id == id }
        if (i < 0) return
        val parent = display[i].parentId
        val next = display.drop(i + 1).firstOrNull { it.parentId == parent }
        val prev = display.take(i).lastOrNull { it.parentId == parent }
        when {
            next != null -> node.moveTaskNextTo(listId, id, next.id, false)
            prev != null -> node.moveTaskNextTo(listId, id, prev.id, true)
            else -> return
        }
        onChanged()
    }

    LazyColumn(modifier, state = lazy) {
        items(display, key = { it.id }) { t ->
            ReorderableItem(reorder, key = t.id) { dragging ->
                Column(Modifier.fillMaxWidth().padding(start = (t.depth.toInt() * 24).dp).background(if (dragging) MaterialTheme.colorScheme.surfaceVariant else MaterialTheme.colorScheme.surface)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text("≡", Modifier.padding(horizontal = 8.dp).draggableHandle(onDragStopped = { drop(t.id) }).testTag("drag-" + t.title))
                        Checkbox(t.done, { done ->
                            node.setDone(listId, t.id, done)
                            if (shopping && done) settling.add(t.id)
                            onChanged()
                        }, modifier = Modifier.testTag("done-" + t.title))
                        if (editing == t.id) {
                            TitleEditor(t.title, t.title) { new ->
                                editing = null
                                if (new.isNotBlank() && new.trim() != t.title) {
                                    node.renameTask(listId, t.id, new.trim())
                                    onChanged()
                                }
                            }
                        } else {
                            Text(t.title, Modifier.weight(1f).clickable { editing = t.id }.padding(vertical = 8.dp).testTag("task-" + t.title))
                        }
                        TextButton(onClick = { onNotes(t.id) }, modifier = Modifier.testTag("notes-" + t.title)) {
                            Text(if (t.notes.isNotEmpty() || t.hasAttachment) "📝" else "🗒")
                        }
                        if (onPickImage != null) TextButton(onClick = {
                            onPickImage { bytes ->
                                runCatching { node.addImage(listId, t.id, bytes) }
                                onChanged()
                            }
                        }, modifier = Modifier.testTag("image-" + t.title)) { Text("🖼") }
                        ReminderButton(node, listId, t.id, t.remindMs, onChanged)
                        TextButton(onClick = { node.addTask(listId, "New subtask", t.id); onChanged() }) { Text("+") }
                        TextButton(onClick = { node.deleteTask(listId, t.id); onChanged() }, modifier = Modifier.testTag("delete-" + t.title)) { Text("✕") }
                    }
                    if (t.images.isNotEmpty()) {
                        Row(Modifier.padding(start = 48.dp, bottom = 4.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            for (img in t.images) Thumbnail(node, listId, img, version, Modifier.clickable { viewing = t.id to img }.testTag("thumb-" + t.title))
                        }
                    }
                }
            }
        }
    }
    // Checked items in a shopping list drop to the bottom after a moment.
    for (id in settling.toList()) {
        LaunchedEffect(id) {
            delay(SETTLE_MS)
            settling.remove(id)
        }
    }
}

@Composable
private fun androidx.compose.foundation.layout.RowScope.TitleEditor(initial: String, key: String, onDone: (String) -> Unit) {
    var text by remember(key) { mutableStateOf(initial) }
    val focus = remember { FocusRequester() }
    var focused by remember { mutableStateOf(false) }
    OutlinedTextField(
        text, { text = it },
        singleLine = true,
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = { onDone(text) }),
        modifier = Modifier.weight(1f).focusRequester(focus).onFocusChanged {
            if (focused && !it.isFocused) onDone(text) // tapped away: keep what was typed
            focused = it.isFocused
        }.testTag("edit-$key"),
    )
    LaunchedEffect(Unit) { focus.requestFocus() }
}

@Composable
private fun image(node: KtNode, listId: String, imageId: String, version: Int): ImageBitmap? {
    val decode = LocalImageDecoder.current
    val bitmap by produceState<ImageBitmap?>(null, imageId, version) {
        value = withContext(Dispatchers.IO) { runCatching { node.taskImage(listId, imageId) }.getOrNull()?.let { decode?.invoke(it) } }
    }
    return bitmap
}

@Composable
private fun Thumbnail(node: KtNode, listId: String, imageId: String, version: Int, modifier: Modifier) {
    val bitmap = image(node, listId, imageId, version)
    val shape = RoundedCornerShape(6.dp)
    if (bitmap != null) {
        Image(bitmap, contentDescription = "Image", contentScale = ContentScale.Crop, modifier = modifier.size(56.dp).clip(shape))
    } else {
        // Still on its way to this device (or not decodable here).
        Box(modifier.size(56.dp).clip(shape).background(MaterialTheme.colorScheme.surfaceVariant), contentAlignment = Alignment.Center) { Text("…") }
    }
}

@Composable
private fun ImageViewer(node: KtNode, listId: String, taskId: String, imageId: String, version: Int, onChanged: () -> Unit, onClose: () -> Unit) {
    val bitmap = image(node, listId, imageId, version)
    AlertDialog(
        onDismissRequest = onClose,
        confirmButton = { TextButton(onClick = onClose) { Text("Close") } },
        dismissButton = {
            TextButton(onClick = { node.removeImage(listId, taskId, imageId); onChanged(); onClose() }, modifier = Modifier.testTag("imageRemove")) { Text("Remove") }
        },
        text = {
            if (bitmap != null) Image(bitmap, contentDescription = "Image", contentScale = ContentScale.Fit, modifier = Modifier.fillMaxWidth().heightIn(max = 480.dp).testTag("imageFull"))
            else Text("This image is still on its way to this device.")
        },
    )
}
