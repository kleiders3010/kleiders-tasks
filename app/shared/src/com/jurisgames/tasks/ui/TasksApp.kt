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
fun TasksApp(
    node: KtNode,
    onRestore: (() -> Unit)? = null,
    onExport: ((ListItem) -> Unit)? = null,
    onShare: ((String) -> Unit)? = null,
    openedCode: String? = null,
    onSettings: ((com.jurisgames.tasks.core.Settings) -> Unit)? = null,
    onScan: ((onResult: (String) -> Unit) -> Unit)? = null,
    onImport: (() -> Unit)? = null,
    update: com.jurisgames.tasks.core.UpdateItem? = null,
    onUpdate: ((String) -> Unit)? = null,
    /** Why this build must update (signed operator manifest only, never peer data). */
    required: String? = null,
    /** Android 12+: wallpaper colors (Material You), for light or dark. */
    dynamicScheme: ((Boolean) -> androidx.compose.material3.ColorScheme?)? = null,
) {
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
    AppTheme(node, version, dynamicScheme) {
        Surface(Modifier.fillMaxSize()) {
            val current = open
            if (current == null) {
                if (required != null) {
                    // Placeholder look (UX pending): stays until this build is updated.
                    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text("Update required: $required", color = MaterialTheme.colorScheme.error, modifier = Modifier.weight(1f).testTag("required"))
                        if (onUpdate != null) TextButton(onClick = { onUpdate(update?.link ?: "https://apps.jurisgames.com/") }, modifier = Modifier.testTag("requiredGo")) { Text("Update") }
                    }
                } else if (update != null && onUpdate != null) {
                    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                        // Principle 8: newer data seen -> say why (text built in; the link only ever
                        // comes from our own update manifest).
                        val why = if (node.newerVersionSeen()) "A newer version is available to use certain features" else "Version ${update.versionName} is available"
                        Text(why, modifier = Modifier.weight(1f).testTag("update"))
                        TextButton(onClick = { onUpdate(update.link) }, modifier = Modifier.testTag("updateGo")) { Text("Update") }
                    }
                }
                ListsScreen(node, version, onRestore, onShare, openedCode, onSettings, onScan, onImport, onChanged = { version++ }, onOpen = { open = it })
            } else {
                TasksScreen(node, current, version, onExport, onChanged = { version++ }, onBack = { open = null })
            }
        }
    }
}

@Composable
private fun ListsScreen(
    node: KtNode,
    version: Int,
    onRestore: (() -> Unit)?,
    onShare: ((String) -> Unit)?,
    openedCode: String?,
    onSettings: ((com.jurisgames.tasks.core.Settings) -> Unit)?,
    onScan: ((onResult: (String) -> Unit) -> Unit)?,
    onImport: (() -> Unit)?,
    onChanged: () -> Unit,
    onOpen: (ListItem) -> Unit,
) {
    var settingsOpen by remember { mutableStateOf(false) }
    if (settingsOpen) SettingsDialog(node, onSaved = { onSettings?.invoke(it); onChanged() }, onClose = { settingsOpen = false })
    val scope = rememberCoroutineScope()
    val lists = remember(node, version) { node.lists() }
    val groups = remember(node, version) { node.groups() }
    var status by remember { mutableStateOf("") }
    var newList by remember { mutableStateOf("") }
    var newGroup by remember { mutableStateOf("") }
    var invite by remember { mutableStateOf<Pair<String, InviteInfo>?>(null) }
    // A link that opened the app goes straight to the matching dialog, prefilled.
    val opened = openedCode?.takeIf { it.isNotBlank() }
    var joining by remember(opened) { mutableStateOf(opened != null && !opened.contains("/link")) }
    var linkCode by remember { mutableStateOf<InviteInfo?>(null) }
    var linkingThis by remember(opened) { mutableStateOf(opened != null && opened.contains("/link")) }
    val devices = remember(node, version) { node.devices() }
    val recent = remember(node, version) { node.recentLinks() }
    val isLinking = remember(node, version) { node.isLinking() }
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
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            TextButton(onClick = { linkCode = node.linkCode() }, modifier = Modifier.testTag("linkDevice")) { Text("Link a device") }
            TextButton(onClick = { settingsOpen = true }, modifier = Modifier.testTag("settingsOpen")) { Text("Settings") }
            if (onImport != null) TextButton(onClick = onImport, modifier = Modifier.testTag("importList")) { Text("Import a list file") }
            if (!isLinking) TextButton(onClick = { linkingThis = true }, modifier = Modifier.testTag("linkThis")) { Text("Link this device") }
        }
        if (isLinking) {
            Text("Linking: this device joins the account as soon as one of its devices is online", style = MaterialTheme.typography.bodySmall, modifier = Modifier.testTag("linkingStatus"))
        } else if (devices.size > 1) {
            Text(
                "Devices: " + devices.joinToString(" · ") { (it.name.ifEmpty { "a device" }) + if (it.thisDevice) " (this one)" else "" },
                style = MaterialTheme.typography.bodySmall,
                modifier = Modifier.testTag("devices"),
            )
        }
        for (l in recent) {
            val who = l.name.ifEmpty { "A device" }
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("New device linked: $who", style = MaterialTheme.typography.bodySmall, modifier = Modifier.weight(1f).testTag("recentLink"))
                TextButton(onClick = { node.unlinkDevice(l.deviceId); onChanged() }, modifier = Modifier.testTag("undo-$who")) { Text("Undo") }
            }
        }
        node.restored()?.let { r ->
            Text(
                "Restored from a backup: ${r.rehomedLists} of your own lists are back" +
                    if (r.rejoiningGroups > 0u) "; ${r.rejoiningGroups} shared groups reopen for editing when a member's device is online" else "",
                style = MaterialTheme.typography.bodySmall,
                modifier = Modifier.testTag("restoredInfo"),
            )
        }
        val oldDeviceGroups = remember(node, version) { node.replacedDeviceGroups() }
        if (oldDeviceGroups > 0u) {
            Column(Modifier.testTag("oldDevice")) {
                Text(
                    "Your previous device is still in $oldDeviceGroups shared ${if (oldDeviceGroups == 1u) "group" else "groups"}. If it was lost or stolen, remove it.",
                    style = MaterialTheme.typography.bodySmall,
                )
                Row {
                    TextButton(onClick = { node.removeReplacedDevice(); onChanged() }, modifier = Modifier.testTag("oldDeviceRemove")) { Text("Remove it") }
                    TextButton(onClick = { node.keepReplacedDevice(); onChanged() }, modifier = Modifier.testTag("oldDeviceKeep")) { Text("Keep it") }
                }
            }
        }
        if (onRestore != null && remember(node, version) { node.isPristine() }) {
            TextButton(onClick = onRestore, modifier = Modifier.testTag("restore")) { Text("Restore from a backup file") }
        }
        HorizontalDivider()
        LazyColumn(Modifier.weight(1f).testTag("lists")) {
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
                    Row(Modifier.fillMaxWidth().clickable { onOpen(l) }.padding(vertical = 10.dp, horizontal = 8.dp).testTag("list-" + l.name), verticalAlignment = Alignment.CenterVertically) {
                        ListBadge(l, Modifier.padding(end = 8.dp))
                        Text(l.name, Modifier.weight(1f))
                    }
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
                    Spacer(Modifier.padding(4.dp))
                    Text("Or the same as a link or QR code:")
                    Text(info.link, style = MaterialTheme.typography.bodySmall, modifier = Modifier.testTag("inviteLink"))
                    QrView(info.link)
                    if (onShare != null) TextButton(onClick = { onShare(info.link) }, modifier = Modifier.testTag("shareInvite")) { Text("Share link") }
                }
            },
        )
    }
    linkCode?.let { info ->
        AlertDialog(
            onDismissRequest = { linkCode = null },
            confirmButton = { TextButton(onClick = { linkCode = null }) { Text("Done") } },
            title = { Text("Link a device") },
            text = {
                Column {
                    Text("On the other device, choose \"Link this device\" and enter these 8 words (valid 4 hours):")
                    Spacer(Modifier.padding(4.dp))
                    Text(info.words.take(4).joinToString(" "), style = MaterialTheme.typography.titleMedium, modifier = Modifier.testTag("linkWords1"))
                    Text(info.words.drop(4).joinToString(" "), style = MaterialTheme.typography.titleMedium, modifier = Modifier.testTag("linkWords2"))
                    Spacer(Modifier.padding(4.dp))
                    Text("Or scan this on the other device:")
                    QrView(info.link)
                }
            },
        )
    }
    if (linkingThis) {
        var words by remember { mutableStateOf(opened?.takeIf { it.contains("/link") } ?: "") }
        var preview by remember { mutableStateOf<String?>(null) }
        var result by remember { mutableStateOf("") }
        val ownLists = remember(node, version) { node.lists().size }
        AlertDialog(
            onDismissRequest = { linkingThis = false },
            confirmButton = {
                if (preview == null) {
                    TextButton(onClick = {
                        result = "Looking…"
                        scope.launch {
                            try {
                                val p = withContext(Dispatchers.IO) { node.linkPreview(words) }
                                preview = "This will join ${p.accountName}'s account: ${p.summary}" +
                                    when (ownLists) {
                                        0 -> "."
                                        1 -> ". The list on this device will be added to it."
                                        else -> ". The $ownLists lists on this device will be added to it."
                                    }
                                result = ""
                            } catch (e: Exception) {
                                result = e.message ?: "Failed"
                            }
                        }
                    }, modifier = Modifier.testTag("linkFind")) { Text("Find") }
                } else {
                    TextButton(onClick = {
                        scope.launch {
                            result = try {
                                withContext(Dispatchers.IO) { node.linkConfirm(words) }
                                onChanged()
                                "Linking: this device joins as soon as one of the account's devices is online"
                            } catch (e: Exception) {
                                e.message ?: "Failed"
                            }
                        }
                    }, modifier = Modifier.testTag("linkGo")) { Text("Link") }
                }
            },
            dismissButton = { TextButton(onClick = { linkingThis = false }) { Text("Close") } },
            title = { Text("Link this device") },
            text = {
                Column {
                    OutlinedTextField(words, { words = it; preview = null }, label = { Text("8 words or the link from your other device") }, modifier = Modifier.testTag("linkThisWords"))
                    if (onScan != null) TextButton(onClick = { onScan { words = it; preview = null } }, modifier = Modifier.testTag("scanLink")) { Text("Scan QR code") }
                    preview?.let { Text(it, modifier = Modifier.testTag("linkPreview")) }
                    if (result.isNotEmpty()) Text(result, modifier = Modifier.testTag("linkResult"))
                }
            },
        )
    }
    if (joining) {
        var words by remember { mutableStateOf(opened?.takeIf { !it.contains("/link") } ?: "") }
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
                    OutlinedTextField(words, { words = it }, label = { Text("8 words (3 letters each is enough) or a link") }, modifier = Modifier.testTag("joinWords"))
                    if (onScan != null) TextButton(onClick = { onScan { words = it } }, modifier = Modifier.testTag("scanJoin")) { Text("Scan QR code") }
                    if (result.isNotEmpty()) Text(result, modifier = Modifier.testTag("joinResult"))
                }
            },
        )
    }
}

@Composable
private fun TasksScreen(node: KtNode, list: ListItem, version: Int, onExport: ((ListItem) -> Unit)?, onChanged: () -> Unit, onBack: () -> Unit) {
    var undoOpen by remember { mutableStateOf(false) }
    if (undoOpen) UndoDialog(node, list, onDone = { onChanged() }, onClose = { undoOpen = false })
    var styleOpen by remember { mutableStateOf(false) }
    var notesFor by remember { mutableStateOf<String?>(null) }
    notesFor?.let { id -> NotesDialog(node, list.id, id, version, onSaved = onChanged, onClose = { notesFor = null }) }
    // The list as it is now (its style may have just changed, here or on another device).
    val shown = remember(node, version, list.id) { node.lists().find { it.id == list.id } ?: list }
    if (styleOpen) StyleDialog(node, shown, onDone = onChanged, onClose = { styleOpen = false })
    val tasks = remember(node, version, list.id) { runCatching { node.tasks(list.id) }.getOrDefault(emptyList()) }
    var title by remember { mutableStateOf("") }
    Column(Modifier.fillMaxSize().padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            TextButton(onClick = onBack, modifier = Modifier.testTag("back")) { Text("‹ Lists") }
            ListBadge(shown, Modifier.padding(end = 8.dp))
            Text("${list.name} · ${list.groupName}", style = MaterialTheme.typography.titleLarge, modifier = Modifier.weight(1f))
            TextButton(onClick = { styleOpen = true }, modifier = Modifier.testTag("styleOpen")) { Text("Style…") }
            if (onExport != null) TextButton(onClick = { onExport(list) }, modifier = Modifier.testTag("export")) { Text("Export") }
            TextButton(onClick = { undoOpen = true }, modifier = Modifier.testTag("undoOpen")) { Text("Undo…") }
        }
        LazyColumn(Modifier.weight(1f)) {
            items(tasks, key = { it.id }) { t ->
                Row(Modifier.fillMaxWidth().padding(start = (t.depth.toInt() * 24).dp), verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(t.done, { node.setDone(list.id, t.id, it); onChanged() }, modifier = Modifier.testTag("done-" + t.title))
                    Text(t.title, Modifier.weight(1f).testTag("task-" + t.title))
                    TextButton(onClick = { notesFor = t.id }, modifier = Modifier.testTag("notes-" + t.title)) {
                        Text(if (t.notes.isNotEmpty() || t.hasAttachment) "📝" else "🗒")
                    }
                    ReminderButton(node, list.id, t.id, t.remindMs, onChanged)
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
