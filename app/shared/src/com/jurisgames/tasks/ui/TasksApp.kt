package com.jurisgames.tasks.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.VerticalDivider
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.jurisgames.tasks.core.InviteInfo
import com.jurisgames.tasks.core.KtListener
import com.jurisgames.tasks.core.KtNode
import com.jurisgames.tasks.core.ListItem
import com.jurisgames.tasks.core.accentColor
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import java.io.File

/**
 * The app: the last list used, ready to type into (docs/UI-SPEC.md). Lists are chips across
 * the top on phones and a sidebar on wide screens; everything else is one tap away in the
 * list's ⋮ menu or the ☰ menu. Everything technical happens in the Rust node.
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
    /** Pick an image (gallery / file) and hand over its compressed bytes. */
    onPickImage: ((onPicked: (ByteArray) -> Unit) -> Unit)? = null,
    /** Turn image bytes into a bitmap (per platform). */
    decodeImage: ((ByteArray) -> androidx.compose.ui.graphics.ImageBitmap?)? = null,
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
    AppTheme(node, version, dynamicScheme) {
        CompositionLocalProvider(LocalImageDecoder provides decodeImage) {
            Surface(Modifier.fillMaxSize()) {
                Home(
                    Env(node, version, { version++ }, onRestore, onExport, onShare, onSettings, onScan, onImport, update, onUpdate, onPickImage, required),
                    openedCode,
                )
            }
        }
    }
}

/** What every part of the screen needs. */
class Env(
    val node: KtNode,
    val version: Int,
    val changed: () -> Unit,
    val onRestore: (() -> Unit)?,
    val onExport: ((ListItem) -> Unit)?,
    val onShare: ((String) -> Unit)?,
    val onSettings: ((com.jurisgames.tasks.core.Settings) -> Unit)?,
    val onScan: ((onResult: (String) -> Unit) -> Unit)?,
    val onImport: (() -> Unit)?,
    val update: com.jurisgames.tasks.core.UpdateItem?,
    val onUpdate: ((String) -> Unit)?,
    val onPickImage: ((onPicked: (ByteArray) -> Unit) -> Unit)?,
    val required: String?,
)

/** The list shown last, per device (a plain file next to the app's data). */
private fun lastListFile(node: KtNode) = File(node.dataDir(), "ui-last-list")

private enum class Dialog { Settings, Join, LinkThis, LinkCode, NewList, Rename, Style, Undo }

@Composable
private fun Home(env: Env, openedCode: String?) {
    val node = env.node
    val scope = rememberCoroutineScope()
    val snackbar = remember { SnackbarHostState() }
    val lists = remember(node, env.version) { node.lists() }
    var currentId by remember { mutableStateOf(runCatching { lastListFile(node).readText().trim() }.getOrDefault("")) }
    val current = lists.find { it.id == currentId } ?: lists.firstOrNull()
    fun open(l: ListItem) {
        currentId = l.id
        runCatching { lastListFile(node).writeText(l.id) }
    }
    val opened = openedCode?.takeIf { it.isNotBlank() }
    var dialog by remember(opened) {
        mutableStateOf(if (opened == null) null else if (opened.contains("/link")) Dialog.LinkThis else Dialog.Join)
    }
    var invite by remember { mutableStateOf<Pair<String, InviteInfo>?>(null) }
    val addFocus = remember { FocusRequester() }

    fun share(l: ListItem) {
        runCatching {
            val shared = node.shareList(l.id)
            open(shared)
            env.changed()
            invite = shared.name to node.invite(shared.groupId, 30u)
        }.onFailure { e -> scope.launch { snackbar.showSnackbar(e.message ?: "Couldn't share") } }
    }

    fun deleteList(l: ListItem) {
        runCatching { node.deleteList(l.id) }
        env.changed()
        scope.launch {
            val r = snackbar.showSnackbar("Deleted ${l.name}", actionLabel = "Undo", duration = SnackbarDuration.Long)
            if (r == SnackbarResult.ActionPerformed) {
                runCatching { node.restoreList(l.groupId, l.id) }
                open(l)
                env.changed()
            }
        }
    }

    val menu = MenuActions(
        join = { dialog = Dialog.Join },
        linkThis = { dialog = Dialog.LinkThis },
        linkCode = { dialog = Dialog.LinkCode },
        settings = { dialog = Dialog.Settings },
    )
    val listMenu = ListActions(
        share = { current?.let(::share) },
        rename = { dialog = Dialog.Rename },
        style = { dialog = Dialog.Style },
        undo = { dialog = Dialog.Undo },
        delete = { current?.let(::deleteList) },
    )

    val keys = Modifier.onPreviewKeyEvent { e ->
        if (e.type != KeyEventType.KeyDown || !e.isCtrlPressed) return@onPreviewKeyEvent false
        when (e.key) {
            Key.N -> { runCatching { addFocus.requestFocus() }; true }
            Key.One, Key.Two, Key.Three, Key.Four, Key.Five, Key.Six, Key.Seven, Key.Eight, Key.Nine -> {
                val i = listOf(Key.One, Key.Two, Key.Three, Key.Four, Key.Five, Key.Six, Key.Seven, Key.Eight, Key.Nine).indexOf(e.key)
                lists.getOrNull(i)?.let(::open)
                true
            }
            else -> false
        }
    }

    BoxWithConstraints(Modifier.fillMaxSize().then(keys)) {
        val wide = maxWidth >= 720.dp
        if (wide) {
            Scaffold(snackbarHost = { SnackbarHost(snackbar) }) { pad ->
                Row(Modifier.fillMaxSize().padding(pad)) {
                    Column(Modifier.width(280.dp).fillMaxHeight().background(MaterialTheme.colorScheme.surfaceContainer)) {
                        Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(vertical = 12.dp)) {
                            Text("Lists", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = 20.dp, vertical = 8.dp))
                            for (l in lists) SidebarRow(l, l.id == current?.id) { open(l) }
                            SidebarRow(null, false, "New list") { dialog = Dialog.NewList }
                        }
                        HorizontalDivider()
                        MenuPanel(env, menu, compact = true)
                    }
                    VerticalDivider()
                    Column(Modifier.weight(1f).fillMaxHeight()) {
                        Banners(env)
                        if (current != null) ListPane(env, current, lists, wide = true, onMenu = null, listMenu = listMenu, snackbar = snackbar, addFocus = addFocus)
                        else FirstList(env, addFocus, onLinkThis = menu.linkThis, onMenu = null, open = ::open)
                    }
                }
            }
        } else {
            val drawer = rememberDrawerState(DrawerValue.Closed)
            ModalNavigationDrawer(drawerState = drawer, drawerContent = {
                ModalDrawerSheet(Modifier.width(300.dp)) {
                    MenuPanel(env, MenuActions(
                        join = { scope.launch { drawer.close() }; menu.join() },
                        linkThis = { scope.launch { drawer.close() }; menu.linkThis() },
                        linkCode = { scope.launch { drawer.close() }; menu.linkCode() },
                        settings = { scope.launch { drawer.close() }; menu.settings() },
                    ), compact = false)
                }
            }) {
                Scaffold(snackbarHost = { SnackbarHost(snackbar) }) { pad ->
                    Column(Modifier.fillMaxSize().padding(pad).imePadding()) {
                        Banners(env)
                        if (current != null) {
                            ListPane(env, current, lists, wide = false, onMenu = { scope.launch { drawer.open() } }, listMenu = listMenu, snackbar = snackbar, addFocus = addFocus) {
                                ListChips(lists, current, ::open) { dialog = Dialog.NewList }
                            }
                        } else {
                            FirstList(env, addFocus, onLinkThis = menu.linkThis, onMenu = { scope.launch { drawer.open() } }, open = ::open)
                        }
                    }
                }
            }
        }
    }

    when (dialog) {
        Dialog.Settings -> SettingsDialog(node, onSaved = { env.onSettings?.invoke(it); env.changed() }, onClose = { dialog = null })
        Dialog.Join -> JoinDialog(env, opened?.takeIf { !it.contains("/link") }, onJoined = { name ->
            node.lists().lastOrNull { it.groupName == name }?.let(::open)
        }, onClose = { dialog = null })
        Dialog.LinkThis -> LinkThisDialog(env, opened?.takeIf { it.contains("/link") }, scope) { dialog = null }
        Dialog.LinkCode -> runCatching { node.linkCode() }.getOrNull()?.let { info ->
            CodeDialog("Link a device", "On your other device, open ☰ › Link this device, then enter this code or scan it.", info, env.onShare) { dialog = null }
        }
        Dialog.NewList -> NewListDialog(env, onCreated = { open(it); dialog = null }, onClose = { dialog = null })
        Dialog.Rename -> current?.let { l -> RenameDialog(l.name, onDone = { n -> runCatching { node.renameList(l.id, n) }; env.changed(); dialog = null }, onClose = { dialog = null }) }
        Dialog.Style -> current?.let { l -> StyleDialog(node, l, onDone = env.changed, onClose = { dialog = null }) }
        Dialog.Undo -> current?.let { l -> UndoDialog(node, l, onDone = env.changed, onClose = { dialog = null }) }
        null -> {}
    }
    invite?.let { (name, info) -> CodeDialog("Invite to $name", "Anyone with this link or code can join.", info, env.onShare) { invite = null } }
}

class MenuActions(val join: () -> Unit, val linkThis: () -> Unit, val linkCode: () -> Unit, val settings: () -> Unit)
class ListActions(val share: () -> Unit, val rename: () -> Unit, val style: () -> Unit, val undo: () -> Unit, val delete: () -> Unit)

@Composable
private fun SidebarRow(l: ListItem?, selected: Boolean, label: String? = null, onClick: () -> Unit) {
    val bg = if (selected) MaterialTheme.colorScheme.secondaryContainer else androidx.compose.ui.graphics.Color.Transparent
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 8.dp).background(bg, RoundedCornerShape(24.dp)).clickable(onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 10.dp).testTag(if (l != null) "list-" + l.name else "newList"),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (l != null) ListBadge(l, Modifier.padding(end = 10.dp)) else Icon(Ic.Add, null, Modifier.padding(end = 10.dp).size(18.dp))
        Text(l?.name ?: label.orEmpty(), maxLines = 1, overflow = TextOverflow.Ellipsis, fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal)
    }
}

@Composable
private fun ListChips(lists: List<ListItem>, current: ListItem, open: (ListItem) -> Unit, newList: () -> Unit) {
    Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        for (l in lists) {
            FilterChip(l.id == current.id, { open(l) }, label = { Text(l.name, maxLines = 1) }, leadingIcon = { ListBadge(l) }, modifier = Modifier.testTag("list-" + l.name))
        }
        FilterChip(false, newList, label = { Text("New list") }, leadingIcon = { Icon(Ic.Add, null, Modifier.size(18.dp)) }, modifier = Modifier.testTag("newList"))
    }
}

/** The ☰ menu: who you are, joining, devices, files, settings. */
@Composable
private fun MenuPanel(env: Env, a: MenuActions, compact: Boolean) {
    val node = env.node
    var name by remember(node, env.version) { mutableStateOf(node.displayName()) }
    val devices = remember(node, env.version) { node.devices() }
    val isLinking = remember(node, env.version) { node.isLinking() }
    Column(Modifier.padding(vertical = 8.dp)) {
        if (!compact) Text("Tasks", style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(start = 20.dp, top = 12.dp, bottom = 8.dp))
        OutlinedTextField(
            name, { name = it; runCatching { node.setDisplayName(it.trim()) } },
            label = { Text("Your name") }, singleLine = true,
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp).testTag("name"),
        )
        MenuRow("Join a shared list", "join", a.join)
        if (isLinking) {
            Text("Linking: this device joins your other one as soon as it's online", style = MaterialTheme.typography.bodySmall, modifier = Modifier.padding(horizontal = 20.dp, vertical = 6.dp).testTag("linkingStatus"))
        } else {
            MenuRow(if (devices.size > 1) "Your devices (${devices.size}) · Add a device" else "Use on another device", "linkDevice", a.linkCode)
            if (devices.size <= 1) MenuRow("Link to your other device", "linkThis", a.linkThis)
        }
        env.onImport?.let { MenuRow("Import a list file", "importList", it) }
        if (env.onRestore != null && remember(node, env.version) { node.isPristine() }) MenuRow("Restore from a backup", "restore", env.onRestore)
        MenuRow("Settings", "settingsOpen", a.settings)
    }
}

@Composable
private fun MenuRow(label: String, tag: String, onClick: () -> Unit) {
    Text(label, Modifier.fillMaxWidth().clickable(onClick = onClick).padding(horizontal = 20.dp, vertical = 12.dp).testTag(tag))
}

/** Things that need a moment of attention: updates, a restore, a new device. Shown above the list. */
@Composable
private fun Banners(env: Env) {
    val node = env.node
    @Composable
    fun Banner(text: String, tag: String, vararg actions: Pair<String, () -> Unit>) {
        Row(Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.secondaryContainer).padding(horizontal = 16.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(text, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f).testTag(tag))
            for ((label, act) in actions) TextButton(onClick = act, modifier = Modifier.testTag("$tag-$label")) { Text(label) }
        }
    }
    val update = env.update
    if (env.required != null) {
        Banner("Please update to keep using shared lists: ${env.required}", "required", *listOfNotNull(env.onUpdate?.let { u -> "Update" to { u(update?.link ?: "https://apps.jurisgames.com/") } }).toTypedArray())
    } else if (update != null && env.onUpdate != null) {
        val why = if (node.newerVersionSeen()) "A newer version is available to use certain features" else "Version ${update.versionName} is available"
        Banner(why, "update", "Update" to { env.onUpdate.invoke(update.link) })
    }
    for (l in remember(node, env.version) { node.recentLinks() }) {
        Banner("New device linked: ${l.name.ifEmpty { "a device" }}", "recentLink", "Undo" to { node.unlinkDevice(l.deviceId); env.changed() })
    }
    node.restored()?.let { r ->
        Banner(
            "Restored from a backup: ${r.rehomedLists} of your lists are back" + if (r.rejoiningGroups > 0u) "; shared lists open for editing once someone in them is online" else "",
            "restoredInfo",
        )
    }
    val old = remember(node, env.version) { node.replacedDeviceGroups() }
    if (old > 0u) {
        Banner(
            "Your previous device can still see $old shared ${if (old == 1u) "list" else "lists"}. Lost or stolen?", "oldDevice",
            "Remove it" to { node.removeReplacedDevice(); env.changed() }, "Keep it" to { node.keepReplacedDevice(); env.changed() },
        )
    }
}

/** A list: title bar, its items, the add field. [chips] goes under the title bar (phones). */
@Composable
private fun ListPane(
    env: Env,
    list: ListItem,
    lists: List<ListItem>,
    wide: Boolean,
    onMenu: (() -> Unit)?,
    listMenu: ListActions,
    snackbar: SnackbarHostState,
    addFocus: FocusRequester,
    chips: (@Composable () -> Unit)? = null,
) {
    val node = env.node
    val look = LocalLook.current
    val accent = argb(accentColor(look.themeId, list.hue, list.vivid, look.dark))
    val groups = remember(node, env.version) { node.groups() }
    val shared = groups.find { it.id == list.groupId }?.personal == false
    val people = remember(node, env.version, list.id) { if (shared) runCatching { node.people(list.id) }.getOrDefault(emptyList()) else emptyList() }
    var menuOpen by remember { mutableStateOf(false) }
    val shopping = list.listType == "shopping"
    Column(Modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().padding(start = 4.dp, end = 4.dp, top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            if (onMenu != null) IconButton(onClick = onMenu, modifier = Modifier.testTag("menu")) { Icon(Ic.Menu, "Menu") } else Spacer(Modifier.width(16.dp))
            Text(
                list.name, style = MaterialTheme.typography.headlineSmall, color = accent, fontWeight = FontWeight.SemiBold,
                maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f).testTag("title"),
            )
            Row(horizontalArrangement = Arrangement.spacedBy((-6).dp)) { for (p in people.filter { !it.isMe }.take(4)) Avatar(p) }
            IconButton(onClick = listMenu.share, modifier = Modifier.testTag("share")) { Icon(Ic.PersonAdd, if (shared) "Invite" else "Share") }
            Box {
                IconButton(onClick = { menuOpen = true }, modifier = Modifier.testTag("listMenu")) { Icon(Ic.More, "More") }
                DropdownMenu(menuOpen, { menuOpen = false }) {
                    @Composable
                    fun item(label: String, tag: String, act: () -> Unit) {
                        DropdownMenuItem(text = { Text(label) }, onClick = { menuOpen = false; act() }, modifier = Modifier.testTag(tag))
                    }
                    item(if (shared) "Invite someone" else "Share…", "menuShare", listMenu.share)
                    item("Rename", "menuRename", listMenu.rename)
                    item("Color and icon", "styleOpen", listMenu.style)
                    item(if (shopping) "Make it a to-do list" else "Make it a shopping list", "shoppingSwitch") {
                        node.setListType(list.id, if (shopping) "tasks" else "shopping"); env.changed()
                    }
                    item("Delete completed", "clearDone") {
                        val done = runCatching { node.tasks(list.id) }.getOrDefault(emptyList()).filter { it.done && it.parentId == null }
                        for (t in done) runCatching { node.deleteTask(list.id, t.id) }
                        env.changed()
                    }
                    env.onExport?.let { ex -> item("Export", "export") { ex(list) } }
                    item("Undo changes…", "undoOpen", listMenu.undo)
                    item("Delete list", "deleteList", listMenu.delete)
                }
            }
        }
        if (shared && people.size > 1) {
            Text(
                "Shared with " + people.filter { !it.isMe }.joinToString { personLabel(it, people) },
                style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1, overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(start = if (onMenu != null) 52.dp else 16.dp, end = 16.dp).testTag("sharedWith"),
            )
        }
        chips?.let { Spacer(Modifier.size(4.dp)); it() }
        ListBody(env, list, lists, shopping, snackbar, addFocus, wide, Modifier.weight(1f))
    }
}

/**
 * First launch: looks like an empty "My tasks" list, ready to type into. The list is only made
 * with the first task, so a fresh install can still restore a backup.
 */
@Composable
private fun FirstList(env: Env, addFocus: FocusRequester, onLinkThis: () -> Unit, onMenu: (() -> Unit)?, open: (ListItem) -> Unit) {
    var text by remember { mutableStateOf("") }
    fun add() {
        val t = text.trim()
        if (t.isEmpty()) return
        val l = runCatching { env.node.createList(null, "My tasks") }.getOrNull() ?: return
        val w = parseWhen(t)
        val id = runCatching { env.node.addTask(l.id, w?.title?.ifBlank { t } ?: t, null) }.getOrNull()
        if (id != null && w != null) runCatching { env.node.setReminder(l.id, id, w.at.millis().toULong()) }
        text = ""
        open(l)
        env.changed()
    }
    Column(Modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().padding(start = 4.dp, top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            if (onMenu != null) IconButton(onClick = onMenu, modifier = Modifier.testTag("menu")) { Icon(Ic.Menu, "Menu") } else Spacer(Modifier.width(16.dp))
            Text("My tasks", style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.SemiBold, modifier = Modifier.testTag("title"))
        }
        Column(Modifier.weight(1f).padding(24.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text("Add your first task below", color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.testTag("empty"))
            Text(
                "Using it on another device? Link this one", color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.bodySmall,
                modifier = Modifier.clickable(onClick = onLinkThis).testTag("firstLink"),
            )
        }
        HorizontalDivider()
        OutlinedTextField(
            text, { text = it }, placeholder = { Text("Add a task") }, singleLine = true,
            keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(imeAction = androidx.compose.ui.text.input.ImeAction.Done),
            keyboardActions = androidx.compose.foundation.text.KeyboardActions(onDone = { add() }),
            trailingIcon = { if (text.isNotBlank()) IconButton(onClick = ::add, modifier = Modifier.testTag("addtask")) { Icon(Ic.Check, "Add") } },
            modifier = Modifier.fillMaxWidth().padding(8.dp).focusRequester(addFocus).testTag("newtask"),
        )
    }
}

@Composable
private fun RenameDialog(name: String, onDone: (String) -> Unit, onClose: () -> Unit) {
    var text by remember { mutableStateOf(name) }
    AlertDialog(
        onDismissRequest = onClose,
        confirmButton = { TextButton(onClick = { if (text.isNotBlank()) onDone(text.trim()) }, modifier = Modifier.testTag("renameGo")) { Text("Rename") } },
        dismissButton = { TextButton(onClick = onClose) { Text("Cancel") } },
        title = { Text("Rename list") },
        text = { OutlinedTextField(text, { text = it }, singleLine = true, modifier = Modifier.testTag("renameText")) },
    )
}

@Composable
private fun NewListDialog(env: Env, onCreated: (ListItem) -> Unit, onClose: () -> Unit) {
    var name by remember { mutableStateOf("") }
    var shopping by remember { mutableStateOf(false) }
    fun create() {
        if (name.isBlank()) return
        val l = runCatching { env.node.createList(null, name.trim()) }.getOrNull() ?: return
        if (shopping) runCatching { env.node.setListType(l.id, "shopping") }
        env.changed()
        onCreated(l)
    }
    AlertDialog(
        onDismissRequest = onClose,
        confirmButton = { TextButton(onClick = ::create, modifier = Modifier.testTag("newListGo")) { Text("Create") } },
        dismissButton = { TextButton(onClick = onClose) { Text("Cancel") } },
        title = { Text("New list") },
        text = {
            Column {
                OutlinedTextField(
                    name, { name = it }, singleLine = true, placeholder = { Text("Name") },
                    keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(imeAction = androidx.compose.ui.text.input.ImeAction.Done),
                    keyboardActions = androidx.compose.foundation.text.KeyboardActions(onDone = { create() }),
                    modifier = Modifier.testTag("newListName"),
                )
                Row(Modifier.clickable { shopping = !shopping }, verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(shopping, { shopping = it }, modifier = Modifier.testTag("newListShopping"))
                    Text("Shopping list (ticked items can be added back with a tap)")
                }
            }
        },
    )
}

/** An invite or link code: copy / share the link, the words with Copy, the QR code on request. */
@Composable
private fun CodeDialog(title: String, hint: String, info: InviteInfo, onShare: ((String) -> Unit)?, onClose: () -> Unit) {
    val clip = androidx.compose.ui.platform.LocalClipboardManager.current
    var qr by remember { mutableStateOf(false) }
    var copied by remember { mutableStateOf("") }
    val until = java.time.format.DateTimeFormatter.ofPattern("d MMM").format(fromMillis(info.expiresMs.toLong()))
    AlertDialog(
        onDismissRequest = onClose,
        confirmButton = { TextButton(onClick = onClose, modifier = Modifier.testTag("codeDone")) { Text("Done") } },
        title = { Text(title) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Text(hint, style = MaterialTheme.typography.bodyMedium)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    androidx.compose.material3.Button(onClick = { clip.setText(androidx.compose.ui.text.AnnotatedString(info.link)); copied = "Link copied" }, modifier = Modifier.testTag("copyLink")) { Text("Copy link") }
                    if (onShare != null) androidx.compose.material3.OutlinedButton(onClick = { onShare(info.link) }, modifier = Modifier.testTag("shareInvite")) { Text("Share") }
                }
                Text("Or send them this code", style = MaterialTheme.typography.labelLarge)
                Text(info.words.joinToString(" - "), style = MaterialTheme.typography.titleMedium, modifier = Modifier.testTag("words"))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    TextButton(onClick = { clip.setText(androidx.compose.ui.text.AnnotatedString(info.words.joinToString(" "))); copied = "Code copied" }, modifier = Modifier.testTag("copyCode")) { Text("Copy code") }
                    TextButton(onClick = { qr = !qr }, modifier = Modifier.testTag("showQr")) { Text(if (qr) "Hide QR code" else "Show QR code") }
                }
                if (qr) Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) { QrView(info.link) }
                Text((if (copied.isNotEmpty()) "$copied · " else "") + "Works until $until", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        },
    )
}

@Composable
private fun LinkThisDialog(env: Env, prefilled: String?, scope: CoroutineScope, onClose: () -> Unit) {
    val node = env.node
    var words by remember { mutableStateOf(prefilled ?: "") }
    var preview by remember { mutableStateOf<String?>(null) }
    var result by remember { mutableStateOf("") }
    val ownLists = remember(node, env.version) { node.lists().size }
    AlertDialog(
        onDismissRequest = onClose,
        confirmButton = {
            if (preview == null) {
                TextButton(onClick = {
                    result = "Looking…"
                    scope.launch {
                        try {
                            val p = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { node.linkPreview(words) }
                            preview = "This joins ${p.accountName}'s lists: ${p.summary}" + when (ownLists) {
                                0 -> "."
                                1 -> ". The list on this device is added to them."
                                else -> ". The $ownLists lists on this device are added to them."
                            }
                            result = ""
                        } catch (e: Exception) {
                            result = e.message ?: "That code didn't work. Check it on your other device."
                        }
                    }
                }, modifier = Modifier.testTag("linkFind")) { Text("Next") }
            } else {
                TextButton(onClick = {
                    scope.launch {
                        result = try {
                            kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { node.linkConfirm(words) }
                            env.changed()
                            "Linking: this device joins as soon as your other one is online"
                        } catch (e: Exception) {
                            e.message ?: "Couldn't link. Try again."
                        }
                    }
                }, modifier = Modifier.testTag("linkGo")) { Text("Link") }
            }
        },
        dismissButton = { TextButton(onClick = onClose) { Text("Close") } },
        title = { Text("Link this device") },
        text = {
            Column {
                OutlinedTextField(words, { words = it; preview = null }, label = { Text("Code or link from your other device") }, modifier = Modifier.testTag("linkThisWords"))
                env.onScan?.let { scan -> TextButton(onClick = { scan { words = it; preview = null } }, modifier = Modifier.testTag("scanLink")) { Text("Scan QR code") } }
                preview?.let { Text(it, modifier = Modifier.testTag("linkPreview")) }
                if (result.isNotEmpty()) Text(result, modifier = Modifier.testTag("linkResult"))
            }
        },
    )
}
