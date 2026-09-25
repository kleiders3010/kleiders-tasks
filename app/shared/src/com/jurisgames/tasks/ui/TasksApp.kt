package com.jurisgames.tasks.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import com.jurisgames.tasks.core.InviteInfo
import com.jurisgames.tasks.core.KtListener
import com.jurisgames.tasks.core.KtNode
import com.jurisgames.tasks.core.ListItem
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Prototype UI (functional, not polished): lists by group, tasks, invites, joining.
 * Everything technical happens in the Rust node; this only shows state and forwards taps.
 */
@Composable
fun TasksApp(node: KtNode, onRestore: (() -> Unit)? = null, onExport: ((ListItem) -> Unit)? = null) {
    // Bumped whenever the node reports a change (another device, membership, compaction).
    var version by remember { mutableIntStateOf(0) }
    DisposableEffect(node) {
        node.setListener(object : KtListener {
            override fun onChange() {
                version++
            }
        })
        onDispose { }
    }
    var open by remember { mutableStateOf<ListItem?>(null) }
    MaterialTheme {
        Surface(Modifier.fillMaxSize()) {
            val current = open
            if (current == null) {
                ListsScreen(node, version, onRestore, onChanged = { version++ }, onOpen = { open = it })
            } else {
                TasksScreen(node, current, version, onExport, onChanged = { version++ }, onBack = { open = null })
            }
        }
    }
}

@Composable
private fun ListsScreen(node: KtNode, version: Int, onRestore: (() -> Unit)?, onChanged: () -> Unit, onOpen: (ListItem) -> Unit) {
    val scope = rememberCoroutineScope()
    val lists = remember(node, version) { node.lists() }
    val groups = remember(node, version) { node.groups() }
    var status by remember { mutableStateOf("") }
    var newList by remember { mutableStateOf("") }
    var newGroup by remember { mutableStateOf("") }
    var invite by remember { mutableStateOf<Pair<String, InviteInfo>?>(null) }
    var joining by remember { mutableStateOf(false) }
    var name by remember(node, version) { mutableStateOf(node.displayName()) }

    Column(Modifier.fillMaxSize().padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(name, { name = it }, label = { Text("Your name") }, modifier = Modifier.weight(1f).testTag("name"))
            TextButton(onClick = { node.setDisplayName(name); onChanged() }) { Text("Save") }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = {
                status = "Syncing…"
                scope.launch {
                    val r = withContext(Dispatchers.IO) { node.syncNow() }
                    status = "Sent ${r.written}, received ${r.received}" + if (r.joinedGroups.isNotEmpty()) " · joined ${r.joinedGroups.joinToString()}" else ""
                    onChanged()
                }
            }, modifier = Modifier.testTag("sync")) { Text("Sync now") }
            Button(onClick = { joining = true }, modifier = Modifier.testTag("join")) { Text("Join with code") }
        }
        if (status.isNotEmpty()) Text(status, style = MaterialTheme.typography.bodySmall, modifier = Modifier.testTag("status"))
        node.restored()?.let { r ->
            Text(
                "Restored from a backup: ${r.rehomedLists} of your own lists are back" +
                    if (r.rejoiningGroups > 0u) "; ${r.rejoiningGroups} shared groups reopen for editing when a member's device is online" else "",
                style = MaterialTheme.typography.bodySmall,
                modifier = Modifier.testTag("restoredInfo"),
            )
        }
        if (onRestore != null && remember(node, version) { node.isPristine() }) {
            TextButton(onClick = onRestore, modifier = Modifier.testTag("restore")) { Text("Restore from a backup file") }
        }
        HorizontalDivider()
        LazyColumn(Modifier.weight(1f)) {
            for (g in groups) {
                item(key = "g" + g.id) {
                    Row(Modifier.fillMaxWidth().padding(top = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text(g.name.ifEmpty { "(group)" }, style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                        if (!g.personal) {
                            TextButton(onClick = { invite = g.name to node.invite(g.id, 30u) }, modifier = Modifier.testTag("invite-" + g.name)) { Text("Invite") }
                        }
                    }
                }
                items(lists.filter { it.groupId == g.id }, key = { it.id }) { l ->
                    Text(
                        l.name,
                        Modifier.fillMaxWidth().clickable { onOpen(l) }.padding(vertical = 10.dp, horizontal = 8.dp).testTag("list-" + l.name),
                    )
                }
                item(key = "n" + g.id) {
                    var n by remember { mutableStateOf("") }
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        OutlinedTextField(n, { n = it }, placeholder = { Text("New list in ${g.name}") }, modifier = Modifier.weight(1f).testTag("newlist-" + g.name))
                        TextButton(onClick = {
                            if (n.isNotBlank()) {
                                node.createList(g.id, n.trim()); n = ""; onChanged()
                            }
                        }, modifier = Modifier.testTag("addlist-" + g.name)) { Text("Add") }
                    }
                }
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(newGroup, { newGroup = it }, placeholder = { Text("New group (share with people)") }, modifier = Modifier.weight(1f).testTag("newgroup"))
            TextButton(onClick = {
                if (newGroup.isNotBlank()) {
                    node.createGroup(newGroup.trim()); newGroup = ""; onChanged()
                }
            }, modifier = Modifier.testTag("addgroup")) { Text("Create") }
        }
    }

    invite?.let { (group, info) ->
        AlertDialog(
            onDismissRequest = { invite = null },
            confirmButton = { TextButton(onClick = { invite = null }) { Text("Done") } },
            title = { Text("Invite to $group") },
            text = {
                Column {
                    Text("Share these 8 words (valid 30 days, any number of people):")
                    Spacer(Modifier.padding(4.dp))
                    Text(info.words.take(4).joinToString(" "), style = MaterialTheme.typography.titleMedium, modifier = Modifier.testTag("words1"))
                    Text(info.words.drop(4).joinToString(" "), style = MaterialTheme.typography.titleMedium, modifier = Modifier.testTag("words2"))
                }
            },
        )
    }
    if (joining) {
        var words by remember { mutableStateOf("") }
        var result by remember { mutableStateOf("") }
        AlertDialog(
            onDismissRequest = { joining = false },
            confirmButton = {
                TextButton(onClick = {
                    result = "Looking…"
                    scope.launch {
                        result = try {
                            val r = withContext(Dispatchers.IO) { node.joinCode(words) }
                            onChanged()
                            if (r.joined) "Joined ${r.groupName}" else "Asked to join ${r.groupName} (${r.inviterName}'s group): you'll be let in automatically"
                        } catch (e: Exception) {
                            e.message ?: "Failed"
                        }
                    }
                }, modifier = Modifier.testTag("joinGo")) { Text("Join") }
            },
            dismissButton = { TextButton(onClick = { joining = false }) { Text("Close") } },
            title = { Text("Join with code") },
            text = {
                Column {
                    OutlinedTextField(words, { words = it }, label = { Text("8 words (3 letters each is enough)") }, modifier = Modifier.testTag("joinWords"))
                    if (result.isNotEmpty()) Text(result, modifier = Modifier.testTag("joinResult"))
                }
            },
        )
    }
}

@Composable
private fun TasksScreen(node: KtNode, list: ListItem, version: Int, onExport: ((ListItem) -> Unit)?, onChanged: () -> Unit, onBack: () -> Unit) {
    val tasks = remember(node, version, list.id) { runCatching { node.tasks(list.id) }.getOrDefault(emptyList()) }
    var title by remember { mutableStateOf("") }
    Column(Modifier.fillMaxSize().padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            TextButton(onClick = onBack, modifier = Modifier.testTag("back")) { Text("‹ Lists") }
            Text("${list.name} · ${list.groupName}", style = MaterialTheme.typography.titleLarge, modifier = Modifier.weight(1f))
            if (onExport != null) TextButton(onClick = { onExport(list) }, modifier = Modifier.testTag("export")) { Text("Export") }
        }
        LazyColumn(Modifier.weight(1f)) {
            items(tasks, key = { it.id }) { t ->
                Row(Modifier.fillMaxWidth().padding(start = (t.depth.toInt() * 24).dp), verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(t.done, { node.setDone(list.id, t.id, it); onChanged() }, modifier = Modifier.testTag("done-" + t.title))
                    Text(t.title, Modifier.weight(1f).testTag("task-" + t.title))
                    TextButton(onClick = { node.addTask(list.id, "New subtask", t.id); onChanged() }) { Text("+") }
                    TextButton(onClick = { node.deleteTask(list.id, t.id); onChanged() }, modifier = Modifier.testTag("delete-" + t.title)) { Text("✕") }
                }
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(title, { title = it }, placeholder = { Text("Add a task") }, modifier = Modifier.weight(1f).testTag("newtask"))
            Spacer(Modifier.width(8.dp))
            Button(onClick = {
                if (title.isNotBlank()) {
                    node.addTask(list.id, title.trim(), null); title = ""; onChanged()
                }
            }, modifier = Modifier.testTag("addtask")) { Text("Add") }
        }
    }
}
