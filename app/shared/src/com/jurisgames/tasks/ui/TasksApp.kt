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
import androidx.compose.foundation.layout.consumeWindowInsets
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
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.foundation.selection.toggleable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
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
import androidx.compose.foundation.layout.heightIn
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
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
    /** Print a page (the list's name, the page as HTML): Android's print service, the desktop's print dialog. */
    onPrint: ((String, String) -> Unit)? = null,
    /** Restore a password-locked backup (its bytes, the checked password). */
    onRestoreWith: ((ByteArray, String) -> Unit)? = null,
    /** Reminders: asked for when one is set, and told when they can't ring (UX U-T1). */
    reminders: ReminderHooks = ReminderHooks(),
    /** This device just joined or shared a list: the moment to ask for change notifications (UX U-T5). */
    onShared: (() -> Unit)? = null,
    /** The platform's Back (Android), given whether it is taken and what it does (cold UX review U-T18). */
    backHandler: (@Composable (Boolean, () -> Unit) -> Unit)? = null,
    /** Write every list to a backup file she picks, the format Restore reads (Ciclo review U-C12, general). */
    onSaveBackup: (() -> Unit)? = null,
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
    var restoring by remember { mutableStateOf<Pair<ByteArray, String>?>(null) }
    AppTheme(node, version, dynamicScheme) {
        CompositionLocalProvider(LocalImageDecoder provides decodeImage) {
            Surface(Modifier.fillMaxSize()) {
                // While a locked backup is restored the old node gets closed: nothing on screen may still read it.
                if (restoring == null) Home(
                    Env(node, version, { version++ }, onRestore, onExport, onShare, onSettings, onScan, onImport, update, onUpdate, onPickImage, required, onPrint, reminders, onShared, backHandler, onSaveBackup),
                    openedCode,
                )
                // A locked backup waits for its password (picked, or restored by Android onto this phone).
                if (onRestoreWith != null) {
                    LaunchedEffect(node) { node.waitingBackup()?.let { PendingRestore.bytes = it } }
                    PendingRestore.bytes?.let { bytes ->
                        RestorePasswordDialog(bytes, onRestore = { pw -> PendingRestore.bytes = null; restoring = bytes to pw }, onCancel = { PendingRestore.bytes = null })
                    }
                    LaunchedEffect(restoring) {
                        val (bytes, pw) = restoring ?: return@LaunchedEffect
                        androidx.compose.runtime.withFrameNanos { } // the screen above is gone first
                        try { onRestoreWith(bytes, pw) } finally { restoring = null }
                    }
                }
            }
        }
    }
}

/** The shared list's sync line and whether it is a warning; null before the first sync round has said anything. */
internal fun syncLine(s: com.jurisgames.tasks.core.GroupSync, firstRoundDone: Boolean): Pair<String, Boolean>? {
    if (s.onlyHere) return tr("Not sent yet: your newest changes are only on this device.") to true
    if (!firstRoundDone) return null
    val others = s.othersLastMs?.let { tr("the others last synced {when}", "when" to whenLabel(fromMillis(it.toLong())).replaceFirstChar { c -> c.lowercase() }) }
        ?: tr("the others haven't synced yet")
    return tr("Sent") + " · " + others to false
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
    val onPrint: ((String, String) -> Unit)? = null,
    val reminders: ReminderHooks = ReminderHooks(),
    val onShared: (() -> Unit)? = null,
    val backHandler: (@Composable (Boolean, () -> Unit) -> Unit)? = null,
    val onSaveBackup: (() -> Unit)? = null,
)

/**
 * What the platform does around reminders. [onSet]: a reminder was just set (Android asks for
 * notifications then, not at the next start). [blocked]: reminders can't ring right now (notifications
 * off), so the details sheet says so. [openSettings]: the app's notification settings.
 */
class ReminderHooks(
    val onSet: () -> Unit = {},
    val blocked: () -> Boolean = { false },
    val openSettings: (() -> Unit)? = null,
)

/** The list shown last, per device (a plain file next to the app's data). */
private fun lastListFile(node: KtNode) = File(node.dataDir(), "ui-last-list")

private enum class Dialog { Settings, Join, LinkThis, LinkCode, NewList, Rename, Style, Undo, Feedback }

@Composable
private fun Home(env: Env, openedCode: String?) {
    val node = env.node
    val scope = rememberCoroutineScope()
    val snackbar = remember { SnackbarHostState() }
    Notice.text?.let { t -> LaunchedEffect(t) { Notice.text = null; snackbar.showSnackbar(t) } }
    // by name only: sharing a list moves it into a group of its own, and that must not move its tab
    val lists = remember(node, env.version) { node.lists().sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.name }) }
    var currentId by remember { mutableStateOf(runCatching { lastListFile(node).readText().trim() }.getOrDefault("")) }
    val current = lists.find { it.id == currentId } ?: lists.firstOrNull()
    // Google Tasks' Starred view: every starred task in one place (shown once something is starred)
    var starredView by remember { mutableStateOf(false) }
    var openFirst by remember { mutableStateOf<String?>(null) }
    val anyStarred = remember(node, env.version, lists) { starredTasks(node, lists).isNotEmpty() }
    // Todoist's Today/Upcoming: every dated task by day (shown once any open task has a date)
    var upcomingView by remember { mutableStateOf(false) }
    val anyDated = remember(node, env.version, lists) { datedTasks(node, lists).isNotEmpty() }
    // Microsoft To Do's "Assigned to me": tasks given to you in shared lists (shown once there are any)
    var assignedView by remember { mutableStateOf(false) }
    val anyAssigned = remember(node, env.version, lists) { assignedTasks(node, lists).isNotEmpty() }
    // Microsoft To Do's search: one field over every list (always there)
    var searchView by remember { mutableStateOf(false) }
    // Microsoft To Do's My Day: what you pick for today, from any list (always there)
    var dayView by remember { mutableStateOf(false) }
    fun showStarred() { starredView = true; upcomingView = false; searchView = false; dayView = false; assignedView = false }
    fun showUpcoming() { upcomingView = true; starredView = false; searchView = false; dayView = false; assignedView = false }
    fun showSearch() { searchView = true; starredView = false; upcomingView = false; dayView = false; assignedView = false }
    fun showDay() { dayView = true; searchView = false; starredView = false; upcomingView = false; assignedView = false }
    fun showAssigned() { assignedView = true; dayView = false; searchView = false; starredView = false; upcomingView = false }
    // My Day adds to the list you were in (not a shopping list: those keep their own items)
    val dayList = current?.takeIf { it.listType != "shopping" } ?: lists.firstOrNull { it.listType != "shopping" }
    fun open(l: ListItem) {
        currentId = l.id
        starredView = false
        upcomingView = false
        searchView = false
        dayView = false
        assignedView = false
        openFirst = null
        runCatching { lastListFile(node).writeText(l.id) }
    }
    // Back from Search, My Day, Upcoming, Starred or Assigned goes to the list, not out of the app (U-T18).
    env.backHandler?.invoke(searchView || dayView || upcomingView || starredView || assignedView) {
        starredView = false; upcomingView = false; searchView = false; dayView = false; assignedView = false
    }
    fun openStarred(l: ListItem, task: String) {
        open(l)
        openFirst = task
    }
    val opened = openedCode?.takeIf { it.isNotBlank() }
    var dialog by remember(opened) {
        mutableStateOf(if (opened == null) null else if (opened.contains("/link")) Dialog.LinkThis else Dialog.Join)
    }
    var invite by remember { mutableStateOf<Pair<String, InviteInfo>?>(null) }
    // A personal list asks first: only "Create invite" makes it shared (cold UX review U-T13).
    var askShare by remember { mutableStateOf<ListItem?>(null) }
    val addFocus = remember { FocusRequester() }
    // After the very first task makes the list, the add field comes back focused: keep typing.
    var focusAdd by remember { mutableStateOf(false) }
    LaunchedEffect(current?.id, focusAdd) {
        if (focusAdd && current != null) {
            kotlinx.coroutines.delay(150)
            runCatching { addFocus.requestFocus() }
            focusAdd = false
        }
    }

    fun share(l: ListItem) {
        runCatching {
            val shared = node.shareList(l.id)
            open(shared)
            env.changed()
            invite = shared.name to node.invite(shared.groupId, 30u)
        }.onFailure { e -> scope.launch { snackbar.showSnackbar(userError(e, tr("Couldn't share. Check your connection and try again."))) } }
    }

    var confirmDelete by remember { mutableStateOf<ListItem?>(null) }

    fun deleteList(l: ListItem, confirmed: Boolean = false) {
        // A shared list goes for everyone in it: the message says so (cold UX review U-T19).
        val shared = node.groups().find { it.id == l.groupId }?.personal == false
        // U-S14 (general): it is the others' list too, so one confirm names who loses it.
        if (shared && !confirmed) { confirmDelete = l; return }
        runCatching { node.deleteList(l.id) }
        env.changed()
        scope.launch {
            val msg = if (shared) tr("Deleted {name} for everyone in it", "name" to l.name) else tr("Deleted {name}", "name" to l.name)
            val r = snackbar.showSnackbar(msg, actionLabel = tr("Undo"), duration = SnackbarDuration.Long)
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
        feedback = { dialog = Dialog.Feedback },
    )
    val listMenu = ListActions(
        share = { current?.let { l -> if (node.groups().find { it.id == l.groupId }?.personal == false) share(l) else askShare = l } },
        rename = { dialog = Dialog.Rename },
        style = { dialog = Dialog.Style },
        undo = { dialog = Dialog.Undo },
        delete = { current?.let(::deleteList) },
    )

    val keys = Modifier.onPreviewKeyEvent { e ->
        if (e.type != KeyEventType.KeyDown || !e.isCtrlPressed) return@onPreviewKeyEvent false
        when (e.key) {
            Key.N -> { runCatching { addFocus.requestFocus() }; true }
            Key.F -> { showSearch(); true }
            // Ctrl+, Settings; Ctrl+Z the Undo of the message on screen, if there is one (else text fields keep it) (U-T25).
            Key.Comma -> { dialog = Dialog.Settings; true }
            Key.Z -> snackbar.currentSnackbarData?.takeIf { it.visuals.actionLabel != null }?.let { it.performAction(); true } ?: false
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
        val roomy = maxWidth >= 1000.dp // room for the details pane beside the list (U-T12)
        if (wide) {
            Scaffold(snackbarHost = { SnackbarHost(snackbar) }) { pad ->
                Row(Modifier.fillMaxSize().padding(pad).consumeWindowInsets(pad).imePadding()) {
                    Column(Modifier.width(280.dp).fillMaxHeight().background(MaterialTheme.colorScheme.surfaceContainer)) {
                        Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(vertical = 12.dp)) {
                            Text(tr("Lists"), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = 20.dp, vertical = 8.dp))
                            ViewRow(Ic.Search, tr("Search"), "searchView", searchView, null) { showSearch() }
                            ViewRow(Ic.Sun, tr("My Day"), "dayView", dayView, SunColor) { showDay() }
                            if (anyDated || upcomingView) ViewRow(Ic.Event, tr("Upcoming"), "upcomingView", upcomingView, null) { showUpcoming() }
                            if (anyStarred || starredView) ViewRow(Ic.Star, tr("Starred"), "starredView", starredView, StarColor) { showStarred() }
                            if (anyAssigned || assignedView) ViewRow(Ic.Person, tr("Assigned to me"), "assignedView", assignedView, null) { showAssigned() }
                            for (l in lists) SidebarRow(l, !starredView && !upcomingView && !searchView && !dayView && !assignedView && l.id == current?.id) { open(l) }
                            SidebarRow(null, false, tr("New list")) { dialog = Dialog.NewList }
                        }
                        HorizontalDivider()
                        MenuPanel(env, menu, compact = true)
                    }
                    VerticalDivider()
                    Column(Modifier.weight(1f).fillMaxHeight()) {
                        Banners(env)
                        if (searchView) SearchPane(env, lists, onMenu = null, onOpen = ::openStarred)
                        else if (dayView) MyDayPane(env, lists, dayList, onMenu = null, onOpen = ::openStarred)
                        else if (upcomingView) UpcomingPane(env, lists, onMenu = null, onOpen = ::openStarred)
                        else if (starredView) StarredPane(env, lists, onMenu = null, onOpen = ::openStarred)
                        else if (assignedView) AssignedPane(env, lists, onMenu = null, onOpen = ::openStarred)
                        // Details beside the list only with room for both (sidebar 280 + list + details 360); narrower
                        // windows open them as a sheet, so the list doesn't shrink to a sliver (cold UX review U-T12).
                        else if (current != null) ListPane(env, current, lists, wide = roomy, onMenu = null, listMenu = listMenu, snackbar = snackbar, addFocus = addFocus, openFirst = openFirst)
                        else FirstList(env, addFocus, onLinkThis = menu.linkThis, onJoin = menu.join, onMenu = null, open = { open(it); focusAdd = true })
                    }
                }
            }
        } else {
            val drawer = rememberDrawerState(DrawerValue.Closed)
            ModalNavigationDrawer(drawerState = drawer, drawerContent = {
                ModalDrawerSheet(Modifier.width(300.dp)) {
                  // The lists and views first, as on wide screens: ☰ is where people look for their lists (U-T7).
                  Column(Modifier.verticalScroll(rememberScrollState())) {
                    fun go(f: () -> Unit) { scope.launch { drawer.close() }; f() }
                    Text("Tasks", style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(start = 20.dp, top = 20.dp, bottom = 8.dp))
                    for (l in lists) SidebarRow(l, !starredView && !upcomingView && !searchView && !dayView && !assignedView && l.id == current?.id, tagPrefix = "drawerList-") { go { open(l) } }
                    SidebarRow(null, false, tr("New list"), tagPrefix = "drawer-") { go { dialog = Dialog.NewList } }
                    ViewRow(Ic.Search, tr("Search"), "drawer-searchView", searchView, null) { go { showSearch() } }
                    ViewRow(Ic.Sun, tr("My Day"), "drawer-dayView", dayView, SunColor) { go { showDay() } }
                    if (anyDated || upcomingView) ViewRow(Ic.Event, tr("Upcoming"), "drawer-upcomingView", upcomingView, null) { go { showUpcoming() } }
                    if (anyStarred || starredView) ViewRow(Ic.Star, tr("Starred"), "drawer-starredView", starredView, StarColor) { go { showStarred() } }
                    if (anyAssigned || assignedView) ViewRow(Ic.Person, tr("Assigned to me"), "drawer-assignedView", assignedView, null) { go { showAssigned() } }
                    HorizontalDivider(Modifier.padding(vertical = 8.dp))
                    MenuPanel(env, MenuActions(
                        join = { scope.launch { drawer.close() }; menu.join() },
                        linkThis = { scope.launch { drawer.close() }; menu.linkThis() },
                        linkCode = { scope.launch { drawer.close() }; menu.linkCode() },
                        settings = { scope.launch { drawer.close() }; menu.settings() },
                        feedback = { scope.launch { drawer.close() }; menu.feedback() },
                    ), compact = true)
                  }
                }
            }) {
                Scaffold(snackbarHost = { SnackbarHost(snackbar) }) { pad ->
                    Column(Modifier.fillMaxSize().padding(pad).consumeWindowInsets(pad).imePadding()) {
                        Banners(env)
                        val viewChips = listOfNotNull(
                            ViewChip(Ic.Search, tr("Search"), "searchView", searchView, null) { showSearch() },
                            ViewChip(Ic.Sun, tr("My Day"), "dayView", dayView, SunColor) { showDay() },
                            if (anyDated || upcomingView) ViewChip(Ic.Event, tr("Upcoming"), "upcomingView", upcomingView, null) { showUpcoming() } else null,
                            if (anyStarred || starredView) ViewChip(Ic.Star, tr("Starred"), "starredView", starredView, StarColor) { showStarred() } else null,
                            if (anyAssigned || assignedView) ViewChip(Ic.Person, tr("Assigned to me"), "assignedView", assignedView, null) { showAssigned() } else null,
                        )
                        if (searchView) {
                            SearchPane(env, lists, onMenu = { scope.launch { drawer.open() } }, onOpen = ::openStarred) {
                                ListChips(lists, null, ::open, viewChips) { dialog = Dialog.NewList }
                            }
                        } else if (dayView) {
                            MyDayPane(env, lists, dayList, onMenu = { scope.launch { drawer.open() } }, onOpen = ::openStarred) {
                                ListChips(lists, null, ::open, viewChips) { dialog = Dialog.NewList }
                            }
                        } else if (upcomingView) {
                            UpcomingPane(env, lists, onMenu = { scope.launch { drawer.open() } }, onOpen = ::openStarred) {
                                ListChips(lists, null, ::open, viewChips) { dialog = Dialog.NewList }
                            }
                        } else if (starredView) {
                            StarredPane(env, lists, onMenu = { scope.launch { drawer.open() } }, onOpen = ::openStarred) {
                                ListChips(lists, null, ::open, viewChips) { dialog = Dialog.NewList }
                            }
                        } else if (assignedView) {
                            AssignedPane(env, lists, onMenu = { scope.launch { drawer.open() } }, onOpen = ::openStarred) {
                                ListChips(lists, null, ::open, viewChips) { dialog = Dialog.NewList }
                            }
                        } else if (current != null) {
                            ListPane(env, current, lists, wide = false, onMenu = { scope.launch { drawer.open() } }, listMenu = listMenu, snackbar = snackbar, addFocus = addFocus, openFirst = openFirst) {
                                ListChips(lists, current, ::open, viewChips) { dialog = Dialog.NewList }
                            }
                        } else {
                            FirstList(env, addFocus, onLinkThis = menu.linkThis, onJoin = menu.join, onMenu = { scope.launch { drawer.open() } }, open = { open(it); focusAdd = true })
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
        // A device link never comes prefilled from a link: linking hands over every list, so the code is typed or
        // scanned here, from the other device's own screen (safety audit round 4, T6).
        Dialog.LinkThis -> LinkThisDialog(env, opened?.contains("/link") == true, scope) { dialog = null }
        Dialog.LinkCode -> runCatching { node.linkCode() }.getOrNull()?.let { info ->
            CodeDialog(tr("Link another device"), tr("On your other device, open ☰ › Link to my other device, then enter this code or scan it."), info, env.onShare) { dialog = null }
        }
        Dialog.NewList -> NewListDialog(env, onCreated = { open(it); dialog = null }, onClose = { dialog = null })
        Dialog.Rename -> current?.let { l -> RenameDialog(l.name, onDone = { n -> runCatching { node.renameList(l.id, n) }.onSuccess { dialog = null }.onFailure { e -> scope.launch { snackbar.showSnackbar(userError(e, tr("Couldn't save. Try again."))) } }; env.changed() }, onClose = { dialog = null }) }
        Dialog.Style -> current?.let { l -> StyleDialog(node, l, onDone = env.changed, onClose = { dialog = null }) }
        Dialog.Undo -> current?.let { l -> UndoDialog(node, l, onDone = env.changed, onClose = { dialog = null }) }
        Dialog.Feedback -> FeedbackDialog(node, onSent = { dialog = null; scope.launch { snackbar.showSnackbar(tr("Thanks! Your message was sent.")) } }, onClose = { dialog = null })
        null -> {}
    }
    askShare?.let { l ->
        androidx.compose.material3.AlertDialog(
            onDismissRequest = { askShare = null },
            title = { Text(tr("Share {name}", "name" to l.name)) },
            text = { Text(tr("People you invite can see and change this list. You get a link and a code to send them.")) },
            confirmButton = { TextButton(onClick = { askShare = null; share(l) }, modifier = Modifier.testTag("shareCreate")) { Text(tr("Create invite")) } },
            dismissButton = { TextButton(onClick = { askShare = null }, modifier = Modifier.testTag("shareCancel")) { Text(tr("Cancel")) } },
        )
    }
    confirmDelete?.let { l ->
        val others = remember(l.id) { runCatching { node.people(l.id) }.getOrDefault(emptyList()).filter { !it.isMe }.map { personLabel(it, emptyList()) }.distinct() }
        AlertDialog(
            onDismissRequest = { confirmDelete = null },
            title = { Text(tr("Delete {name} for everyone?", "name" to l.name)) },
            text = { Text(if (others.isEmpty()) tr("It is shared: it goes from every device that joined it.") else tr("It goes from {people}'s devices too.", "people" to others.joinToString(", "))) },
            confirmButton = { TextButton(onClick = { confirmDelete = null; deleteList(l, confirmed = true) }, modifier = Modifier.testTag("deleteConfirm")) { Text(tr("Delete for everyone"), color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = { confirmDelete = null }, modifier = Modifier.testTag("deleteCancel")) { Text(tr("Cancel")) } },
        )
    }
    invite?.let { (name, info) -> CodeDialog(tr("Invite to {name}", "name" to name), tr("Anyone with this link or code can open this list and join, until it expires. Share it only with the people you invite."), info, env.onShare, node) { invite = null; env.onShared?.invoke() } }
}

class MenuActions(val join: () -> Unit, val linkThis: () -> Unit, val linkCode: () -> Unit, val settings: () -> Unit, val feedback: () -> Unit)
class ListActions(val share: () -> Unit, val rename: () -> Unit, val style: () -> Unit, val undo: () -> Unit, val delete: () -> Unit)

@Composable
private fun SidebarRow(l: ListItem?, selected: Boolean, label: String? = null, tagPrefix: String = "", onClick: () -> Unit) {
    val bg = if (selected) MaterialTheme.colorScheme.secondaryContainer else androidx.compose.ui.graphics.Color.Transparent
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 8.dp).background(bg, RoundedCornerShape(24.dp)).clickable(onClick = onClick)
            .heightIn(min = 48.dp).padding(horizontal = 12.dp, vertical = 10.dp).testTag(if (l != null) (tagPrefix.ifEmpty { "list-" }) + l.name else tagPrefix + "newList"),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (l != null) ListBadge(l, Modifier.padding(end = 10.dp)) else Icon(Ic.Add, null, Modifier.padding(end = 10.dp).size(18.dp))
        Text(l?.name ?: label.orEmpty(), maxLines = 1, overflow = TextOverflow.Ellipsis, fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal)
    }
}

/** A place across lists (Upcoming, Starred) as a phone chip. */
private class ViewChip(val icon: ImageVector, val label: String, val tag: String, val selected: Boolean, val tint: Color?, val onClick: () -> Unit)

@Composable
private fun ViewRow(icon: ImageVector, label: String, tag: String, selected: Boolean, tint: Color?, onClick: () -> Unit) {
    val bg = if (selected) MaterialTheme.colorScheme.secondaryContainer else Color.Transparent
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 8.dp).background(bg, RoundedCornerShape(24.dp)).clickable(onClick = onClick)
            .heightIn(min = 48.dp).padding(horizontal = 12.dp, vertical = 10.dp).testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(icon, null, Modifier.padding(end = 10.dp).size(18.dp), tint = tint ?: MaterialTheme.colorScheme.onSurfaceVariant)
        Text(label, maxLines = 1, fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal)
    }
}

@Composable
private fun ListChips(lists: List<ListItem>, current: ListItem?, open: (ListItem) -> Unit, views: List<ViewChip>, newList: () -> Unit) {
    Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        // The person's own lists first, then the views (cold UX review U-T7: on a narrow phone the views hid every list).
        for (l in lists) {
            FilterChip(l.id == current?.id,{ open(l) }, label = { Text(l.name, maxLines = 1) }, leadingIcon = { ListBadge(l) }, modifier = Modifier.testTag("list-" + l.name))
        }
        for (v in views) {
            FilterChip(v.selected, v.onClick, label = { Text(v.label) }, leadingIcon = { Icon(v.icon, null, Modifier.size(18.dp), tint = v.tint ?: MaterialTheme.colorScheme.onSurfaceVariant) }, modifier = Modifier.testTag(v.tag))
        }
        FilterChip(false, newList, label = { Text(tr("New list")) }, leadingIcon = { Icon(Ic.Add, null, Modifier.size(18.dp)) }, modifier = Modifier.testTag("newList"))
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
            label = { Text(tr("Your name")) }, singleLine = true,
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp).testTag("name"),
        )
        MenuRow(tr("Join a shared list"), "join", a.join)
        if (isLinking) {
            Text(tr("Linking: this device joins your other one as soon as it's online"), style = MaterialTheme.typography.bodySmall, modifier = Modifier.padding(horizontal = 20.dp, vertical = 6.dp).testTag("linkingStatus"))
        } else {
            MenuRow(if (devices.size > 1) tr("Your devices ({n}) · Link another device", "n" to devices.size) else tr("Link another device"), "linkDevice", a.linkCode)
            if (devices.size <= 1) MenuRow(tr("Link to my other device"), "linkThis", a.linkThis)
        }
        env.onImport?.let { MenuRow(tr("Import a list file"), "importList", it) }
        val pristine = remember(node, env.version) { node.isPristine() }
        if (env.onSaveBackup != null && !pristine) MenuRow(tr("Save a backup file"), "saveBackup", env.onSaveBackup)
        if (env.onRestore != null && pristine) MenuRow(tr("Restore from a backup"), "restore", env.onRestore)
        MenuRow(tr("Settings"), "settingsOpen", a.settings)
        // in the menu itself, not only inside Settings (Kleider, 2026-11-21: "Make sure users always have an easy way to give feedback or report bugs on ALL our apps")
        MenuRow(tr("Send feedback"), "feedbackMenu", a.feedback)
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
        Banner(tr("Please update to keep using shared lists: {v}", "v" to env.required), "required", *listOfNotNull(env.onUpdate?.let { u -> tr("Update") to { u(update?.link ?: "https://apps.jurisgames.com/") } }).toTypedArray())
    } else if (update != null && env.onUpdate != null) {
        val why = if (node.newerVersionSeen()) tr("A newer version is available to use certain features") else tr("Version {v} is available", "v" to update.versionName)
        Banner(why, "update", tr("Update") to { env.onUpdate.invoke(update.link) })
    }
    for (l in remember(node, env.version) { node.recentLinks() }) {
        Banner(tr("New device linked: {name}", "name" to l.name.ifEmpty { tr("a device") }), "recentLink", tr("Undo") to { node.unlinkDevice(l.deviceId); env.changed() })
    }
    // A device an invite let in, shown to every member with an undo (second-pass audit T2-13, rule 17).
    for (j in remember(node, env.version) { runCatching { node.recentJoins() }.getOrDefault(emptyList()) }) {
        val who = j.name.ifEmpty { tr("A new device") }
        Banner(
            if (j.by.isNotEmpty()) tr("{who} joined {list}, let in by {by}'s invite", "who" to who, "list" to j.listName, "by" to j.by) else tr("{who} joined {list}", "who" to who, "list" to j.listName),
            "recentJoin",
            tr("Remove") to { runCatching { node.removeJoined(j.groupId, j.deviceId) }; env.changed() },
            tr("OK") to { runCatching { node.dismissJoin(j.groupId, j.deviceId) }; env.changed() },
        )
    }
    node.restored()?.let { r ->
        Banner(
            tr("Restored from a backup: {n} of your lists are back", "n" to r.rehomedLists) + if (r.rejoiningGroups > 0u) tr("; shared lists open for editing once someone in them is online") else "",
            "restoredInfo",
        )
    }
    val old = remember(node, env.version) { node.replacedDeviceGroups() }
    if (old > 0u) {
        Banner(
            (if (old == 1u) tr("Your previous device can still see {n} shared list. Lost or stolen?", "n" to old) else tr("Your previous device can still see {n} shared lists. Lost or stolen?", "n" to old)), "oldDevice",
            tr("Remove it") to { node.removeReplacedDevice(); env.changed() }, tr("Keep it") to { node.keepReplacedDevice(); env.changed() },
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
    openFirst: String? = null,
    chips: (@Composable () -> Unit)? = null,
) {
    val node = env.node
    val look = LocalLook.current
    val accent = argb(accentColor(look.themeId, list.hue, list.vivid, look.dark))
    val groups = remember(node, env.version) { node.groups() }
    val shared = groups.find { it.id == list.groupId }?.personal == false
    val people = remember(node, env.version, list.id) { if (shared) runCatching { node.people(list.id) }.getOrDefault(emptyList()) else emptyList() }
    var menuOpen by remember { mutableStateOf(false) }
    var printing by remember { mutableStateOf(false) }
    val shopping = list.listType == "shopping"
    var sort by remember(list.id) { mutableStateOf(sortOf(node, list.id)) }
    val clearing = remember(list.id) { mutableStateListOf<String>() } // "Delete completed", Undo still possible
    val paneScope = rememberCoroutineScope()
    Column(Modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().padding(start = 4.dp, end = 4.dp, top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            if (onMenu != null) IconButton(onClick = onMenu, modifier = Modifier.testTag("menu")) { Icon(Ic.Menu, tr("Menu")) } else Spacer(Modifier.width(16.dp))
            Text(
                list.name, style = MaterialTheme.typography.headlineSmall, color = accent, fontWeight = FontWeight.SemiBold,
                maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f).testTag("title"),
            )
            Row(horizontalArrangement = Arrangement.spacedBy((-6).dp)) { for (p in people.filter { !it.isMe }.take(4)) Avatar(p) }
            IconButton(onClick = listMenu.share, modifier = Modifier.testTag("share")) { Icon(Ic.PersonAdd, if (shared) tr("Invite") else tr("Share")) }
            Box {
                IconButton(onClick = { menuOpen = true }, modifier = Modifier.testTag("listMenu")) { Icon(Ic.More, tr("More")) }
                DropdownMenu(menuOpen, { menuOpen = false }) {
                    @Composable
                    fun item(label: String, tag: String, act: () -> Unit) {
                        DropdownMenuItem(text = { Text(label) }, onClick = { menuOpen = false; act() }, modifier = Modifier.testTag(tag))
                    }
                    item(if (shared) tr("Invite someone") else tr("Share…"), "menuShare", listMenu.share)
                    item(tr("Rename"), "menuRename", listMenu.rename)
                    item(tr("Color and icon"), "styleOpen", listMenu.style)
                    item(if (shopping) tr("Make it a to-do list") else tr("Make it a shopping list"), "shoppingSwitch") {
                        node.setListType(list.id, if (shopping) "tasks" else "shopping"); env.changed()
                    }
                    if (!shopping) {
                        HorizontalDivider()
                        Text(tr("Sort by"), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp))
                        for (s in Sort.entries) DropdownMenuItem(
                            text = { Text(tr(s.label)) },
                            leadingIcon = { if (s == sort) Icon(Ic.Check, null, Modifier.size(18.dp)) else Spacer(Modifier.size(18.dp)) },
                            onClick = { menuOpen = false; sort = s; setSortOf(node, list.id, s) },
                            modifier = Modifier.testTag("sort-" + s.key),
                        )
                        HorizontalDivider()
                    }
                    // Hidden first, deleted when the Undo snackbar ends (cold UX review U-T3: one tap removed them for everyone).
                    val doneNow = remember(node, env.version, list.id) { runCatching { node.tasks(list.id) }.getOrDefault(emptyList()).filter { it.done && it.parentId == null && it.id !in clearing } }
                    if (doneNow.isNotEmpty()) item(tr("Delete completed ({n})", "n" to doneNow.size.toString()), "clearDone") {
                        val ids = doneNow.map { it.id }
                        clearing.addAll(ids)
                        paneScope.launch {
                            val r = snackbar.showSnackbar((if (ids.size == 1) tr("Deleted 1 completed") else tr("Deleted {n} completed", "n" to ids.size.toString())), actionLabel = tr("Undo"), duration = SnackbarDuration.Long)
                            if (r != SnackbarResult.ActionPerformed) for (id in ids) runCatching { node.deleteTask(list.id, id) }
                            clearing.removeAll(ids)
                            env.changed()
                        }
                    }
                    env.onExport?.let { ex -> item(tr("Export"), "export") { ex(list) } }
                    if (env.onPrint != null) item(tr("Print list"), "printOpen") { printing = true }
                    item(if (shared) tr("Undo changes…") else tr("Undo recent changes…"), "undoOpen", listMenu.undo)
                    HorizontalDivider()
                    DropdownMenuItem(
                        text = { Text(if (shared) tr("Delete list for everyone") else tr("Delete list"), color = MaterialTheme.colorScheme.error) },
                        leadingIcon = { Icon(Ic.Delete, null, tint = MaterialTheme.colorScheme.error) },
                        onClick = { menuOpen = false; listMenu.delete() }, modifier = Modifier.testTag("deleteList"),
                    )
                }
            }
        }
        if (shared && people.size <= 1) {
            // Cold UX review U-S26 (general): shared, but nobody has joined yet.
            Row(Modifier.padding(start = if (onMenu != null) 52.dp else 16.dp, end = 8.dp).testTag("waitingJoin"), verticalAlignment = Alignment.CenterVertically) {
                Text(tr("Invite made. Waiting for someone to join."), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.weight(1f, fill = false))
                TextButton(onClick = listMenu.share) { Text(tr("Show invite")) }
            }
        }
        if (shared && people.size > 1) {
            Text(
                tr("Shared with ") + people.filter { !it.isMe }.joinToString { personLabel(it, people) },
                style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1, overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(start = if (onMenu != null) 52.dp else 16.dp, end = 16.dp).testTag("sharedWith"),
            )
            // Whether the others can have it yet (cold UX review U-T4): read again every 15 s while the list is open.
            val sync by androidx.compose.runtime.produceState<com.jurisgames.tasks.core.GroupSync?>(null, node, list.groupId, env.version) {
                while (true) {
                    value = runCatching { node.groupSync(list.groupId) }.getOrNull()
                    kotlinx.coroutines.delay(15_000)
                }
            }
            sync?.let { s -> syncLine(s, node.firstRoundDone())?.let { (text, warn) ->
                Text(
                    text, style = MaterialTheme.typography.bodySmall,
                    color = if (warn) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 2, overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(start = if (onMenu != null) 52.dp else 16.dp, end = 16.dp)
                        .semantics { liveRegion = LiveRegionMode.Polite }.testTag("syncLine"),
                )
            } }
        }
        if (printing) env.onPrint?.let { print ->
            PrintDialog(node.dataDir(), onPrint = { choice ->
                val tasks = runCatching { node.tasks(list.id) }.getOrDefault(emptyList())
                print(list.name, printHtml(list.name, tasks, if (shopping) Sort.Order else sort, choice))
            }, onDismiss = { printing = false })
        }
        chips?.let { Spacer(Modifier.size(4.dp)); it() }
        ListBody(env, list, lists, shopping, snackbar, addFocus, wide, Modifier.weight(1f), sort, openFirst, clearing)
    }
}

/**
 * First launch: looks like an empty "My tasks" list, ready to type into. The list is only made
 * with the first task, so a fresh install can still restore a backup.
 */
@Composable
private fun FirstList(env: Env, addFocus: FocusRequester, onLinkThis: () -> Unit, onJoin: () -> Unit, onMenu: (() -> Unit)?, open: (ListItem) -> Unit) {
    var text by androidx.compose.runtime.saveable.rememberSaveable { mutableStateOf("") }
    var ignoreWhen by remember { mutableStateOf(false) }
    val parsed = if (ignoreWhen) null else parseWhen(text)
    fun add() {
        val t = text.trim()
        if (t.isEmpty()) return
        val l = runCatching { env.node.createList(null, tr("My tasks")) }.getOrNull() ?: return
        val w = parsed
        val id = runCatching { env.node.addTask(l.id, w?.title?.ifBlank { t } ?: t, null) }.getOrNull()
        if (id != null && w != null) runCatching { env.node.setReminder(l.id, id, w.at.millis().toULong()) }
        if (id != null && w?.repeat != null) runCatching { env.node.setRepeat(l.id, id, w.repeat) }
        if (id != null && w != null) env.reminders.onSet()
        text = ""
        open(l)
        env.changed()
    }
    Column(Modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().padding(start = 4.dp, top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            if (onMenu != null) IconButton(onClick = onMenu, modifier = Modifier.testTag("menu")) { Icon(Ic.Menu, tr("Menu")) } else Spacer(Modifier.width(16.dp))
            Text(tr("My tasks"), style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.SemiBold, modifier = Modifier.testTag("title"))
        }
        Column(Modifier.weight(1f).padding(24.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            // Each a full 48 dp tap (U-T14); joining is offered here too, for a code given by voice or text (U-T17).
            Text(
                tr("Using Tasks on another device? Link to it"), color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.heightIn(min = 48.dp).clickable(onClick = onLinkThis).padding(vertical = 12.dp).testTag("firstLink"),
            )
            Text(
                tr("Got an invite code? Join a shared list"), color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.heightIn(min = 48.dp).clickable(onClick = onJoin).padding(vertical = 12.dp).testTag("firstJoin"),
            )
        }
        HorizontalDivider()
        Box(Modifier.padding(horizontal = 8.dp, vertical = 6.dp)) {
            AddField(text, { text = it; if (it.isEmpty()) ignoreWhen = false }, parsed, { ignoreWhen = true }, tr("Add a task"), addFocus, ::add)
        }
    }
}

@Composable
private fun RenameDialog(name: String, onDone: (String) -> Unit, onClose: () -> Unit) {
    var text by remember { mutableStateOf(name) }
    AlertDialog(
        onDismissRequest = onClose,
        confirmButton = { TextButton(onClick = { if (text.isNotBlank()) onDone(text.trim()) }, modifier = Modifier.testTag("renameGo")) { Text(tr("Rename")) } },
        dismissButton = { TextButton(onClick = onClose) { Text(tr("Cancel")) } },
        title = { Text(tr("Rename list")) },
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
        confirmButton = { TextButton(onClick = ::create, modifier = Modifier.testTag("newListGo")) { Text(tr("Create")) } },
        dismissButton = { TextButton(onClick = onClose) { Text(tr("Cancel")) } },
        title = { Text(tr("New list")) },
        text = {
            Column {
                OutlinedTextField(
                    name, { name = it }, singleLine = true, placeholder = { Text(tr("Name")) },
                    keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(imeAction = androidx.compose.ui.text.input.ImeAction.Done),
                    keyboardActions = androidx.compose.foundation.text.KeyboardActions(onDone = { create() }),
                    modifier = Modifier.testTag("newListName"),
                )
                Row(Modifier.toggleable(shopping, role = androidx.compose.ui.semantics.Role.Checkbox) { shopping = it }.heightIn(min = 48.dp).testTag("newListShopping"), verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(shopping, null)
                    Text(tr("Shopping list (ticked items can be added back with a tap)"))
                }
            }
        },
    )
}

/** U-S11 (general): a shared list shows who did what, so a person without a name is asked where sharing happens. */
@Composable
internal fun NameAsk(node: KtNode) {
    val had = remember(node) { node.displayName().isNotBlank() }
    if (had) return
    var name by remember { mutableStateOf("") }
    OutlinedTextField(name, { name = it; runCatching { node.setDisplayName(it.trim()) } }, singleLine = true,
        label = { Text(tr("Your name (so the others know who did what)")) }, modifier = Modifier.fillMaxWidth().testTag("askName"))
}

/** An invite or link code: copy / share the link, the words with Copy, the QR code on request. */
@Composable
private fun CodeDialog(title: String, hint: String, info: InviteInfo, onShare: ((String) -> Unit)?, nameNode: KtNode? = null, onClose: () -> Unit) {
    val clip = androidx.compose.ui.platform.LocalClipboardManager.current
    var qr by remember { mutableStateOf(false) }
    var copied by remember { mutableStateOf("") }
    val until = java.time.format.DateTimeFormatter.ofPattern("d MMM", Lang.locale).format(fromMillis(info.expiresMs.toLong())).let(::shortName)
    AlertDialog(
        onDismissRequest = onClose,
        confirmButton = { TextButton(onClick = onClose, modifier = Modifier.testTag("codeDone")) { Text(tr("Done")) } },
        title = { Text(title) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                nameNode?.let { NameAsk(it) }
                Text(hint, style = MaterialTheme.typography.bodyMedium)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    androidx.compose.material3.Button(onClick = { clip.setText(androidx.compose.ui.text.AnnotatedString(info.link)); copied = tr("Link copied") }, modifier = Modifier.testTag("copyLink")) { Text(tr("Copy link")) }
                    if (onShare != null) androidx.compose.material3.OutlinedButton(onClick = { onShare(info.link) }, modifier = Modifier.testTag("shareInvite")) { Text(tr("Share")) }
                }
                Text(tr("Or send them this code"), style = MaterialTheme.typography.labelLarge)
                Text(info.words.joinToString(" - "), style = MaterialTheme.typography.titleMedium, modifier = Modifier.testTag("words"))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    TextButton(onClick = { clip.setText(androidx.compose.ui.text.AnnotatedString(info.words.joinToString(" "))); copied = tr("Code copied") }, modifier = Modifier.testTag("copyCode")) { Text(tr("Copy code")) }
                    TextButton(onClick = { qr = !qr }, modifier = Modifier.testTag("showQr")) { Text(if (qr) tr("Hide QR code") else tr("Show QR code")) }
                }
                if (qr) Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) { QrView(info.link) }
                Text((if (copied.isNotEmpty()) "$copied · " else "") + tr("Works until {date}", "date" to until), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        },
    )
}

@Composable
private fun LinkThisDialog(env: Env, fromLink: Boolean, scope: CoroutineScope, onClose: () -> Unit) {
    val node = env.node
    var words by remember { mutableStateOf("") }
    var preview by remember { mutableStateOf<String?>(null) }
    var result by remember { mutableStateOf("") }
    val ownLists = remember(node, env.version) { node.lists().size }
    AlertDialog(
        onDismissRequest = onClose,
        confirmButton = {
            if (preview == null) {
                TextButton(onClick = {
                    result = tr("Looking…")
                    scope.launch {
                        try {
                            val p = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { node.linkPreview(words) }
                            // Fixed words say what is handed over; the other side's own name and summary are shown as
                            // theirs (in quotes), never as the app's wording.
                            preview = tr("Linking gives that device everything: all the lists on this device, and a place in every list you share. Only go on if you made this code yourself on your other device.") +
                                "\n" + tr("It calls itself “{name}” ({summary})", "name" to p.accountName, "summary" to trSummary(p.summary)) + when (ownLists) {
                                0 -> "."
                                1 -> tr(". The list on this device is added to them.")
                                else -> tr(". The {n} lists on this device are added to them.", "n" to ownLists)
                            }
                            result = ""
                        } catch (e: Exception) {
                            result = userError(e, tr("That code didn't work. Check it on your other device."))
                        }
                    }
                }, modifier = Modifier.testTag("linkFind")) { Text(tr("Next")) }
            } else {
                TextButton(onClick = {
                    scope.launch {
                        result = try {
                            kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { node.linkConfirm(words) }
                            env.changed()
                            tr("Linking: this device joins as soon as your other one is online")
                        } catch (e: Exception) {
                            userError(e, tr("Couldn't link. Try again."))
                        }
                    }
                }, modifier = Modifier.testTag("linkGo")) { Text(tr("Link")) }
            }
        },
        dismissButton = { TextButton(onClick = onClose) { Text(tr("Close")) } },
        title = { Text(tr("Link to my other device")) },
        text = {
            Column {
                if (fromLink) Text(tr("For safety, a device is linked only with the code shown on your other device: type it or scan it here."), modifier = Modifier.testTag("linkFromLink"))
                OutlinedTextField(words, { words = it; preview = null }, label = { Text(tr("Code or link from your other device")) }, modifier = Modifier.testTag("linkThisWords"))
                env.onScan?.let { scan -> TextButton(onClick = { scan { words = it; preview = null } }, modifier = Modifier.testTag("scanLink")) { Text(tr("Scan QR code")) } }
                preview?.let { Text(it, modifier = Modifier.testTag("linkPreview")) }
                if (result.isNotEmpty()) Text(result, modifier = Modifier.testTag("linkResult"))
            }
        },
    )
}
