package org.fluxchat.companion.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.material.icons.outlined.History
import androidx.compose.material.icons.outlined.AttachFile
import androidx.compose.material.icons.outlined.Mic
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.outlined.Description
import androidx.compose.material.icons.outlined.Image
import androidx.compose.material.icons.outlined.PhotoCamera
import androidx.compose.material.icons.outlined.Videocam
import androidx.compose.material.icons.outlined.ViewAgenda
import androidx.compose.material.icons.automirrored.outlined.Chat
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalWindowInfo
import org.fluxchat.companion.launchHopping
import org.fluxchat.companion.matrix.Attachment
import org.fluxchat.companion.matrix.SessionList
import org.fluxchat.companion.matrix.Uploads
import androidx.compose.material3.rememberDrawerState
import androidx.compose.material3.PermanentDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.DrawerValue
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.automirrored.outlined.OpenInNew
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.PushPin
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material.icons.outlined.PushPin
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.SmallFloatingActionButton
import androidx.compose.material3.SuggestionChip
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.movableContentOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.async
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import org.fluxchat.companion.agents.BusyMode
import org.fluxchat.companion.agents.Harness
import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.agents.RunInfo
import org.fluxchat.companion.agents.RunState
import org.fluxchat.companion.agents.Runs
import org.fluxchat.companion.agents.SendAction
import org.fluxchat.companion.agents.ViewKey
import org.fluxchat.companion.matrix.AgentList
import org.fluxchat.companion.matrix.Matrix
import org.fluxchat.companion.matrix.TimelineModel

/** How long after you send a request its thread opens by itself when the agent starts on it. */
private const val FOLLOW_WINDOW_MS = 3 * 60 * 1000L
/** A run counts as started by a request sent at most this long before the run's own timestamp. */
private const val FOLLOW_SLACK_MS = 5_000L

/**
 * The session to open by itself: one the agent has started on for a request
 * sent from this screen at [sentAt] (0: none was), not followed yet, while
 * that's recent. Null: nothing to follow.
 */
internal fun followTarget(runs: Map<ViewKey, RunInfo?>, sentAt: Long, followed: Set<String>, now: Long): String? {
    if (sentAt == 0L || now - sentAt >= FOLLOW_WINDOW_MS) return null
    return runs.entries.firstOrNull { (k, r) ->
        k.threadRoot != null && r != null && r.state.live && r.since >= sentAt - FOLLOW_SLACK_MS && k.threadRoot !in followed
    }?.key?.threadRoot
}

/** A room's timeline, or a thread's (`root`): messages, the run, the composer. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ChatScreen(roomId: String, root: String?, go: (Screen) -> Unit, back: () -> Unit, swap: (Screen) -> Unit = go, jumpTo: String? = null) {
    val shared = remember(roomId, root) { if (root == null) Matrix.roomTimeline(roomId) else null }
    // Threads (and rooms that aren't kept open) get their own timeline while on screen.
    var openFailed by remember(roomId, root) { mutableStateOf(false) }
    val own by produceState<TimelineModel?>(null, roomId, root) {
        // (the state outlives a change of keys: never the previous view's, closed, timeline)
        value = null
        if (shared != null) return@produceState
        // Opened on the app's scope and handed over: opened in this one, a
        // screen left while the open was in flight dropped the timeline that
        // then arrived without closing it — its listener ran until sign-out.
        val opening = Matrix.scope.async {
            // Null (a project you've left, opened from Run history) or a throw
            // used to mean a spinner for ever, or a crash.
            try {
                if (root != null) Matrix.openThread(roomId, root) else Matrix.openLive(roomId)
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Exception) {
                org.fluxchat.companion.Diag.error("Opening ${root ?: roomId}", e)
                null
            }
        }
        val m = try {
            opening.await()
        } catch (e: kotlinx.coroutines.CancellationException) {
            Matrix.launch { opening.await()?.close() }
            throw e
        }
        if (m == null) openFailed = true
        value = m
        // Closed by the producer that opened it (not by an effect keyed on
        // `own`, which needs one more composition to exist — the screen could
        // go in between — and not whatever `own` is by then).
        awaitDispose { m?.close() }
    }
    val model = shared ?: own

    val empty = remember { MutableStateFlow(emptyList<Msg>()) }
    val own_ by (model?.messages ?: empty).collectAsState()
    // A thread: the room's live timeline has its replies too (and ones the
    // thread's own timeline misses: those that came in with the room's
    // history before it opened). Both, by event.
    val roomModel = remember(roomId, root) { if (root != null) Matrix.roomTimeline(roomId) else null }
    val roomAll by (roomModel?.messages ?: empty).collectAsState()
    val all = remember(own_, roomAll, root) {
        if (root == null || roomAll.isEmpty()) own_
        else {
            val fromRoom = Runs.viewMessages(roomAll, ViewKey(roomId, root))
            val ids = fromRoom.mapTo(HashSet()) { it.id }
            (fromRoom + own_.filter { it.id !in ids }).sortedBy { it.ts }
        }
    }
    val rooms by Matrix.agentRooms.collectAsState()
    val list by Matrix.agentList.collectAsState()
    val now by rememberNow(5_000)
    val agentRoom = rooms[roomId]
    val agents = agentRoom?.agents ?: Matrix.agentsInRoom(roomId)
    val key = ViewKey(roomId, root)

    // A thread timeline's replies may not name their root: they're all its.
    val msgs = remember(all, root) {
        if (root == null) Runs.viewMessages(all, key) else all.map { if (it.id == root || it.threadRoot == root) it else it.copy(threadRoot = root) }
    }
    val run: RunInfo? = remember(all, agentRoom, now) {
        if (root == null) agentRoom?.runs?.get(key)
        else Runs.roomRuns(roomId, msgs, agents, list::harnessOf, Matrix.isDm(roomId), now).runs[key]
            // A run the agent fell silent on is over here too (the project
            // list already drops it): a 👀 left behind showed "Working" with
            // Stop, and sent what you typed as `!steer` to an agent with no run.
            ?.takeUnless { Runs.stalled(it, Matrix.signOfLife(roomId, it.agent, msgs, root, it.since), now) }
            ?: agentRoom?.runs?.get(key)
    }
    val agent = run?.agent?.takeIf { it.isNotEmpty() } ?: agents.firstOrNull()
    val harness = list.harnessOf(agent ?: "")
    // A room where every request gets its own thread reads as its sessions,
    // a card each; the plain timeline is a tap away.
    val threaded = root == null && agent != null && !Matrix.isDm(roomId) && Runs.repliesInThreads(msgs, agent, harness, false)
    // Arriving at one message (the gallery's "go to message"): the timeline has it, the cards don't.
    var timelineView by rememberSaveable(roomId) { mutableStateOf(jumpTo != null) }
    val cards = threaded && !timelineView
    val sessionList by produceState<SessionList?>(null, roomId, threaded) { if (threaded) value = Matrix.sessions(roomId) }
    // In a room: a run of yours going on in a thread (its card opens it).
    val threadRun: Pair<ViewKey, RunInfo>? = if (root == null && (run == null || !run.state.live)) {
        agentRoom?.runs?.mapNotNull { (k, r) -> if (k.threadRoot != null && r != null && r.state.live) k to r else null }
            ?.sortedWith(compareBy({ it.second.rank().first }, { it.second.rank().second }))?.firstOrNull()
    } else null

    // Follow: a request sent from here whose thread the agent has started on.
    var sentAt by rememberSaveable { mutableStateOf(0L) }
    var followed by rememberSaveable { mutableStateOf(setOf<String>()) }
    // The composer has something in it (text, an attachment, a shot being taken).
    var composing by remember { mutableStateOf(false) }
    LaunchedEffect(agentRoom?.runs) {
        if (root != null) return@LaunchedEffect
        val target = followTarget(agentRoom?.runs.orEmpty(), sentAt, followed, System.currentTimeMillis()) ?: return@LaunchedEffect
        // Decided once per session, either way: not opened now, it doesn't
        // jump at you later when you clear the box.
        followed = followed + target
        // Typing the next request: being pulled into the session the last
        // one started took the keyboard and the half-written text with it.
        if (!composing) go(Screen.Thread(roomId, target))
    }

    // Read only while it's actually being looked at: with sync kept alive in
    // the background, every new message was marked read from a locked phone.
    val watching = LocalWindowInfo.current.isWindowFocused && unlocked()
    LaunchedEffect(msgs.size, watching) { if (watching) model?.markRead() }

    val title = if (root == null) Matrix.roomName(roomId) else threadTitle(msgs.firstOrNull { it.id == root }, msgs.filter { it.id != root })
    var menu by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    // Sends outlive the screen (the app's scope, not this composition's):
    // leaving right after Send cancelled an upload and deleted its file.
    val app = context.applicationContext
    val sendRaw: (String) -> Unit = { text ->
        Matrix.launch {
            if (!send(model, roomId, root, text)) say(app, "Couldn't send “${text.take(40)}”. Check your connection.")
        }
    }
    // What the composer sent and nothing would take: back to it (see [Unsent]).
    val unsent by Unsent.all.collectAsState()
    // Sends that haven't gone out, and what's to be done about them.
    val onSendAction: (String, SendAction) -> Unit = { id, action -> Matrix.launch { sendAction(app, model, roomId, id, action) } }
    // The send queue goes in order: a send that failed for good holds up
    // everything sent after it in the project, sessions included.
    val blocker = remember(all, roomAll) { sendBlocker(if (root == null) all else roomAll.ifEmpty { all }) }
    val answers by Answers.all.collectAsState()
    val command: (String) -> Unit = { name ->
        val c = harness.command(name)
        sendRaw(if (harness.mentionCommands && agent != null && !Matrix.isDm(roomId)) "$agent $c" else c)
    }
    val scroll = TopAppBarDefaults.pinnedScrollBehavior()

    // Sessions (the room's threads): a drawer on phones, a sidebar on wide screens.
    val drawer = rememberDrawerState(DrawerValue.Closed)
    val hasSessions = agents.isNotEmpty() && !cards
    val openSession: (String) -> Unit = { r ->
        scope.launch { drawer.close() }
        if (r != root) if (root == null) go(Screen.Thread(roomId, r)) else swap(Screen.Thread(roomId, r))
    }
    val newSession: () -> Unit = {
        scope.launch { drawer.close() }
        when {
            // A DM has no threads: the agent's own "new session" command.
            Matrix.isDm(roomId) && harness.hasCommand("new") -> command("new")
            // Elsewhere a new request in the room starts one (its thread opens).
            root != null -> swap(Screen.Room(roomId))
        }
    }

    val content: @Composable (Boolean) -> Unit = { sessionsButton -> Scaffold(
        modifier = Modifier.nestedScroll(scroll.nestedScrollConnection),
        topBar = {
            TopAppBar(
                navigationIcon = { IconButton(onClick = back) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") } },
                title = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        if (root == null) {
                            Avatar(roomId, title, Matrix.roomAvatar(roomId), 36.dp, room = !Matrix.isDm(roomId))
                            Spacer(Modifier.width(12.dp))
                        }
                        Column {
                            Text(title, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.titleMedium)
                            val typing = typingNames(roomId, all)
                            val sub = when {
                                run != null && run.state.live -> runLine(run, now)
                                typing != null -> typing
                                run != null -> runLine(run, now)
                                root != null -> "Session in ${Matrix.roomName(roomId)}"
                                else -> agents.joinToString { a -> shortName(a, all.lastOrNull { it.sender == a }?.senderName) }
                            }
                            Text(
                                sub, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.labelMedium,
                                color = run?.state?.color() ?: MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                },
                actions = {
                    if (root != null) {
                        // Pin: keep this session on top of the list and its project on the Projects screen.
                        val pins by Matrix.pins.collectAsState()
                        val sessionPinned = pins.isPinned(roomId, root)
                        IconButton(onClick = { scope.launch { setPinnedOrSay(context, roomId, root, !sessionPinned) } }) {
                            if (sessionPinned) Icon(Icons.Default.PushPin, "Unpin session")
                            else Icon(Icons.Outlined.PushPin, "Pin session")
                        }
                    }
                    if (threaded) {
                        IconButton(onClick = { timelineView = !timelineView }) {
                            if (cards) Icon(Icons.AutoMirrored.Outlined.Chat, "Show the timeline")
                            else Icon(Icons.Outlined.ViewAgenda, "Show sessions")
                        }
                    }
                    if (sessionsButton) {
                        IconButton(onClick = { scope.launch { drawer.open() } }) { Icon(Icons.Outlined.History, "Sessions") }
                    }
                    IconButton(onClick = { go(Screen.Gallery(roomId, root)) }) { Icon(Icons.Outlined.Image, "Gallery") }
                    // One menu: the harness's commands, plus leaving a project
                    // you started (a DM with an agent has nothing to leave; a
                    // thread isn't a separate project).
                    if (harness.menu.isNotEmpty() || (root == null && !Matrix.isDm(roomId))) {
                        var confirmLeave by remember { mutableStateOf(false) }
                        Box {
                            IconButton(onClick = { menu = true }) { Icon(Icons.Default.MoreVert, "Agent commands") }
                            DropdownMenu(expanded = menu && unlocked(), onDismissRequest = { menu = false }, shape = MaterialTheme.shapes.medium) {
                                for (item in harness.menu) {
                                    DropdownMenuItem(text = { Text(item.label) }, onClick = { menu = false; command(item.command) })
                                }
                                if (root == null && !Matrix.isDm(roomId)) {
                                    DropdownMenuItem(text = { Text("Leave project", color = MaterialTheme.colorScheme.error) }, onClick = { menu = false; confirmLeave = true })
                                }
                            }
                        }
                        if (confirmLeave && unlocked()) {
                            AlertDialog(
                                onDismissRequest = { confirmLeave = false },
                                title = { Text("Leave this project?") },
                                text = { Text("It stays for your agents and on desktop fluxchat; it just leaves your phone.") },
                                confirmButton = {
                                    TextButton(onClick = {
                                        confirmLeave = false
                                        scope.launch {
                                            if (Matrix.leaveProject(roomId)) back()
                                            else {
                                                Matrix.clearError()
                                                android.widget.Toast.makeText(context, "Couldn't leave the project. Check your connection.", android.widget.Toast.LENGTH_LONG).show()
                                            }
                                        }
                                    }) { Text("Leave") }
                                },
                                dismissButton = { TextButton(onClick = { confirmLeave = false }) { Text("Cancel") } },
                            )
                        }
                    }
                },
                scrollBehavior = scroll,
            )
        },
    ) { pad ->
        Column(Modifier.fillMaxSize().padding(top = pad.calculateTopPadding()).imePadding().navigationBarsPadding()) {
            Box(Modifier.weight(1f)) {
                if (cards) {
                    SessionFeed(roomId, all, agents, agentRoom?.runs.orEmpty(), sessionList, now, onSendAction, blocker) { r -> go(Screen.Thread(roomId, r)) }
                } else if (model == null) {
                    Box(Modifier.fillMaxSize().padding(32.dp), contentAlignment = Alignment.Center) {
                        if (openFailed) Text(
                            "Couldn't open this. It may no longer be on this phone.",
                            style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant,
                        ) else CircularProgressIndicator()
                    }
                } else {
                    // A session (a thread, or a DM with the agent) reads as a transcript.
                    val transcript = agents.isNotEmpty() && (root != null || Matrix.isDm(roomId))
                    MessageList(model, msgs, root, agents, list, now, go, roomId, transcript, run, jumpTo, onSendAction, blocker, answers)
                }
            }
            // The parked send is somewhere else (another session, or the
            // project's own timeline): from here there'd be only messages
            // that never leave "sending", and no reason why.
            val elsewhere = blocker?.takeIf { b -> if (cards) b.threadRoot != null else msgs.none { it.id == b.id } }
            if (elsewhere != null) {
                HeldUpCard(inSession = elsewhere.threadRoot != null) {
                    val there = elsewhere.threadRoot?.let { Screen.Thread(roomId, it) } ?: Screen.Room(roomId)
                    if (root == null) go(there) else swap(there)
                }
            }
            if (run != null && (run.state.live || now - run.since < 60_000)) {
                RunCard(run, now, harness.stop.isNotEmpty() && run.state.live, onStop = { command(harness.stop) }, onOpen = null)
            } else if (threadRun != null && !cards) {
                val rootMsg = all.firstOrNull { it.id == threadRun.first.threadRoot }
                RunCard(threadRun.second, now, false, onStop = {}, onOpen = {
                    go(Screen.Thread(roomId, threadRun.first.threadRoot!!))
                }, label = threadTitle(rootMsg, emptyList()))
            }
            Composer(
                harness = harness,
                working = run?.state == RunState.Working,
                placeholder = if (cards) "Start a new session" else null,
                canAttach = model != null,
                shareFor = roomId,
                onAttach = { a, caption ->
                    if (root == null) sentAt = System.currentTimeMillis()
                    Matrix.launch {
                        if (model != null && sendAttachment(app, model, a, caption)) {
                            // The upload has its own copy: a camera shot's (or a shared file's) staged one can go.
                            Uploads.release(app, a)
                        } else {
                            if (model == null) say(app, "Couldn't send ${a.name}")
                            Unsent.add(key, Unsent.Item(caption, BusyMode.Send, a))
                        }
                    }
                },
                onSend = { text, mode ->
                    val out = Runs.outgoing(harness, text, run?.state == RunState.Working, mode, agent?.takeIf { !Matrix.isDm(roomId) })
                    if (root == null) sentAt = System.currentTimeMillis()
                    org.fluxchat.companion.quick.Quick.remember(context, roomId)
                    Matrix.launch {
                        if (!send(model, roomId, root, out)) {
                            // As typed (not `out`): it goes through here again.
                            Unsent.add(key, Unsent.Item(text, mode))
                            say(app, "Couldn't send your message. It's kept by the message box: check your connection and send it again.")
                        }
                    }
                },
                onDrafting = { composing = it },
                unsent = unsent[key]?.firstOrNull(),
                onUnsentTaken = { Unsent.remove(key, it) },
            )
        }
    } }

    // One screen, moved between the three layouts below rather than rebuilt
    // in each: flipping to the timeline (or the phone unfolding) threw away
    // what you had typed, the attachment and the scroll position.
    val latest by rememberUpdatedState(content)
    val screen = remember { movableContentOf<Boolean> { sessionsButton -> latest(sessionsButton) } }
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val wide = maxWidth >= WIDE_LAYOUT
        when {
            !hasSessions -> screen(false)
            wide -> Row(Modifier.fillMaxSize()) {
                PermanentDrawerSheet(Modifier.width(SESSIONS_WIDTH)) { SessionsPane(roomId, root, openSession, newSession) }
                Box(Modifier.weight(1f)) { screen(false) }
            }
            else -> ModalNavigationDrawer(
                drawerState = drawer,
                // The left edge is Android's back gesture: the drawer opens from its button.
                gesturesEnabled = drawer.isOpen,
                drawerContent = { ModalDrawerSheet(drawerState = drawer) { SessionsPane(roomId, root, openSession, newSession) } },
            ) { screen(true) }
        }
    }
}

/** Pages that add no row before paging gives up until you scroll again. */
private const val MAX_EMPTY_PAGES = 5
/** How long a page's rows take to land in the list. */
private const val PAGE_SETTLE_MS = 250L
/** Pages a deep link pulls looking for its message before the list opens normally. */
private const val MAX_JUMP_PAGES = 60

/** Wider than this, sessions sit in a sidebar beside the chat (tablets, foldables, landscape). */
private val WIDE_LAYOUT = 840.dp
private val SESSIONS_WIDTH = 320.dp

@Composable
private fun MessageList(
    model: TimelineModel,
    msgs: List<Msg>,
    root: String?,
    agents: List<String>,
    list: AgentList,
    now: Long,
    go: (Screen) -> Unit,
    roomId: String,
    transcript: Boolean = false,
    run: RunInfo? = null,
    jumpTo: String? = null,
    onSendAction: (String, SendAction) -> Unit = { _, _ -> },
    /** The project's earliest parked send: what's still "sending" after it waits for it. */
    blocker: Msg? = null,
    answers: Map<String, Answers.Entry> = emptyMap(),
) {
    val context = LocalContext.current
    val me = remember { Matrix.userId }
    val state = rememberLazyListState()
    val scope = rememberCoroutineScope()
    val atStart by model.atStart.collectAsState()
    var loading by remember { mutableStateOf(false) }
    val nearTop by remember { derivedStateOf { (state.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0) >= state.layoutInfo.totalItemsCount - 4 } }
    val awayFromBottom by remember { derivedStateOf { state.firstVisibleItemIndex > 2 } }
    val currentMsgs by rememberUpdatedState(msgs)
    LaunchedEffect(nearTop, atStart) {
        if (!nearTop || atStart) return@LaunchedEffect
        loading = true
        try {
            // Keep paging while you sit at the top: a page of edits and
            // reactions adds no row, and waiting for the row count to change
            // read as the start of history. `finally`: scrolling away while a
            // page was in flight left the spinner up and paging off for good.
            var empty = 0
            while (empty < MAX_EMPTY_PAGES) {
                val before = currentMsgs.size
                if (!model.paginateBack()) break
                delay(PAGE_SETTLE_MS)
                if (currentMsgs.size == before) empty++ else empty = 0
            }
        } finally {
            loading = false
        }
    }
    val reversed = remember(msgs) { msgs.asReversed() }
    // A search deep-link: hold off following the newest until the hit has
    // been scrolled to (it may still need older pages pulled in).
    var jumped by remember(jumpTo) { mutableStateOf(jumpTo == null) }
    FollowNewest(state, msgs, enabled = jumped)
    // The step log the working run is on: its activity block spins.
    val liveSteps = remember(msgs, run?.state) {
        if (run?.state != RunState.Working) null
        else msgs.lastOrNull { it.sender in agents && Runs.progressSteps(it.body) != null }?.id
    }
    // Agents' own status reactions (👀 ✅ ❌): the run card and title say it.
    val statusReaction: (org.fluxchat.companion.agents.Reaction) -> Boolean = { r ->
        r.users.isNotEmpty() && r.users.all { u ->
            u in agents && list.harnessOf(u).let { h -> r.emoji in setOf(h.workingReaction, h.doneReaction, h.failedReaction) - "" }
        }
    }
    // A session as turns: finished work folded under one line, the answer standing out.
    val entries = remember(msgs, run?.state, now / 60_000, transcript) {
        if (!transcript) emptyList()
        else transcriptEntries(msgs, agents, live = run?.state?.live == true) { i ->
            val m = msgs[i]
            m.sender in agents && (Runs.openPrompt(msgs, i, list.harnessOf(m.sender), now).isNotEmpty() ||
                Runs.pendingQuestion(msgs, i, m.sender, list.harnessOf(m.sender), now) != null)
        }.asReversed()
    }
    // A search hit deep-linked in: scroll to the matching message (pulling
    // older pages until it loads), then hand the list back to FollowNewest.
    val hitInEntry: (TranscriptEntry, String) -> Boolean = { e, id ->
        when (e) {
            is TranscriptEntry.Row -> e.id == id
            is TranscriptEntry.Work -> e.indices.any { msgs.getOrNull(it)?.id == id }
            is TranscriptEntry.Answer -> e.id == id || msgs.getOrNull(e.answerIndex)?.id == id ||
                e.askedIndex?.let { msgs.getOrNull(it)?.id } == id ||
                e.work?.indices?.any { msgs.getOrNull(it)?.id == id } == true
            is TranscriptEntry.Done -> msgs.getOrNull(e.answerIndex)?.id == id
        }
    }
    var jumpPages by remember(jumpTo) { mutableStateOf(0) }
    LaunchedEffect(jumpTo, msgs.size, entries, atStart, jumpPages) {
        if (jumpTo == null || jumped) return@LaunchedEffect
        val at = if (transcript) entries.indexOfFirst { hitInEntry(it, jumpTo) }
        else reversed.indexOfFirst { it.id == jumpTo }
        when {
            at >= 0 -> { state.scrollToItem(at); jumped = true }
            // Everything loaded (or enough tried) and it's not here: open normally.
            atStart || jumpPages >= MAX_JUMP_PAGES -> jumped = true
            else -> {
                // The hit isn't loaded yet: older pages. Counted, so a page
                // that adds nothing (or fails) is followed by another instead
                // of leaving the list never following the newest again.
                model.paginateBack()
                delay(PAGE_SETTLE_MS)
                jumpPages++
            }
        }
    }
    @Composable
    fun transcriptRow(i: Int, prev: Msg?) {
        val m = msgs[i]
        val isAgent = m.sender in agents
        val h = list.harnessOf(m.sender)
        val prompt = if (isAgent) Runs.openPrompt(msgs, i, h, now) else emptyList()
        val question = if (isAgent) Runs.pendingQuestion(msgs, i, m.sender, h, now) else null
        TranscriptRow(
            m = m,
            prev = prev,
            isAgent = isAgent,
            hiddenReactions = statusReaction,
            answered = if (isAgent && prompt.isEmpty()) Runs.answeredApproval(m, h, agents) else null,
            prompt = prompt,
            question = question,
            live = m.id == liveSteps,
            onReact = { key -> if (prompt.any { it.first == key }) answerPrompt(context, roomId, m.id, key) else Matrix.launch { react(model, roomId, m.id, key) } },
            onAnswer = { text -> answerQuestion(context, model, roomId, root, m.id, text) },
            answer = if (prompt.isEmpty()) null else promptAnswer(m, prompt, me, answers[m.id]),
            questionAnswer = if (question == null) null else questionAnswer(msgs, i, answers[m.id], now),
            onSendAction = { onSendAction(m.id, it) },
            now = now,
            heldUp = heldUpBy(blocker, m),
        )
    }
    Box(Modifier.fillMaxSize()) {
        LazyColumn(state = state, reverseLayout = true, contentPadding = PaddingValues(vertical = 8.dp), modifier = Modifier.fillMaxSize()) {
            if (transcript) items(entries, key = { it.key }) { e ->
                when (e) {
                    is TranscriptEntry.Row -> Column {
                        val before = msgs.getOrNull(e.index - 1)
                        val newDay = before == null || !sameDay(before.ts, msgs[e.index].ts)
                        if (newDay) DaySeparator(msgs[e.index].ts)
                        transcriptRow(e.index, if (newDay) null else e.prev)
                    }
                    is TranscriptEntry.Answer -> Column {
                        val a = msgs[e.answerIndex]
                        val before = msgs.getOrNull((e.work?.indices?.first() ?: e.answerIndex) - 1)
                        if (before == null || !sameDay(before.ts, a.ts)) DaySeparator(a.ts)
                        // ✅ / ❌ from the agent on the request, else (the latest turn) the run's state.
                        val asked = e.askedIndex?.let { msgs[it] }
                        val outcome = asked?.let { Runs.requestState(list.harnessOf(a.sender), it, a.sender) }?.takeIf { !it.live }
                            ?: run?.state?.takeIf { e.done.latest && !it.live }
                            ?: RunState.Done
                        TranscriptAnswer(e, a, run, outcome) {
                            e.work?.indices?.forEachIndexed { n, i -> transcriptRow(i, if (n == 0) null else msgs[e.work.indices[n - 1]]) }
                        }
                    }
                    is TranscriptEntry.Work, is TranscriptEntry.Done -> {}
                }
            } else itemsIndexed(reversed, key = { _, m -> m.id }) { ri, m ->
                val i = msgs.size - 1 - ri
                val prev = msgs.getOrNull(i - 1)
                val next = msgs.getOrNull(i + 1)
                val newDay = prev == null || !sameDay(prev.ts, m.ts)
                val continued = !newDay && prev.sender == m.sender && m.ts - prev.ts < 5 * 60 * 1000 && prev.isMessage
                val lastInGroup = next == null || next.sender != m.sender || next.ts - m.ts >= 5 * 60 * 1000 || !sameDay(m.ts, next.ts)
                val isAgent = m.sender in agents
                val h = list.harnessOf(m.sender)
                val prompt = if (isAgent) Runs.openPrompt(msgs, i, h, now) else emptyList()
                val question = if (isAgent) Runs.pendingQuestion(msgs, i, m.sender, h, now) else null
                Column {
                    if (newDay) DaySeparator(m.ts)
                    MessageRow(
                        m = m,
                        continued = continued,
                        lastInGroup = lastInGroup,
                        isAgent = isAgent,
                        inThread = root != null,
                        prompt = prompt,
                        question = question,
                        onReact = { key -> if (prompt.any { it.first == key }) answerPrompt(context, roomId, m.id, key) else Matrix.launch { react(model, roomId, m.id, key) } },
                        onAnswer = { text -> answerQuestion(context, model, roomId, root, m.id, text) },
                        onOpenThread = if (root == null && m.hasThread) ({ go(Screen.Thread(roomId, m.id)) }) else null,
                        threadRun = if (root == null) Matrix.agentRooms.value[roomId]?.runs?.get(ViewKey(roomId, m.id)) else null,
                        now = now,
                        answer = if (prompt.isEmpty()) null else promptAnswer(m, prompt, me, answers[m.id]),
                        questionAnswer = if (question == null) null else questionAnswer(msgs, i, answers[m.id], now),
                        onSendAction = { onSendAction(m.id, it) },
                        heldUp = heldUpBy(blocker, m),
                    )
                }
            }
            if (loading) item { Box(Modifier.fillMaxWidth().padding(16.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(Modifier.size(24.dp), strokeWidth = 2.dp) } }
        }
        AnimatedVisibility(
            awayFromBottom,
            enter = scaleIn() + fadeIn(),
            exit = scaleOut() + fadeOut(),
            modifier = Modifier.align(Alignment.BottomEnd).padding(16.dp),
        ) {
            SmallFloatingActionButton(onClick = { scope.launch { state.animateScrollToItem(0) } }) {
                Icon(Icons.Default.KeyboardArrowDown, "Jump to latest")
            }
        }
    }
}

/**
 * Send text to the view: through its timeline (local echo, thread relation),
 * else straight to the room (in the thread).
 */
private suspend fun send(model: TimelineModel?, roomId: String, root: String?, text: String): Boolean {
    val viaTimeline = model != null && try {
        model.send(text)
        true
    } catch (e: kotlinx.coroutines.CancellationException) {
        throw e
    } catch (e: Exception) {
        org.fluxchat.companion.Diag.error("Sending through the timeline in $roomId", e)
        false
    }
    if (viaTimeline || Matrix.sendText(roomId, root, text, inReplyTo = null)) return true
    // Neither took it: the caller says so (and keeps the text).
    org.fluxchat.companion.Diag.error("Couldn't send to $roomId")
    return false
}

/** A toast, from whatever thread a send ended on. */
private suspend fun say(context: android.content.Context, text: String) {
    kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.Main) {
        android.widget.Toast.makeText(context, text, android.widget.Toast.LENGTH_LONG).show()
    }
}

/** Upload a picked file to the view. False: it couldn't be handed over (said in a toast, and Diagnostics). */
private suspend fun sendAttachment(context: android.content.Context, model: TimelineModel, a: Attachment, caption: String): Boolean = try {
    model.sendAttachment(context, a, caption)
    true
} catch (e: kotlinx.coroutines.CancellationException) {
    throw e
} catch (e: Exception) {
    org.fluxchat.companion.Diag.error("Sending ${a.name} (${a.mime}) to ${model.roomId}", e)
    say(context, "Couldn't send ${a.name}: ${e.message ?: e.javaClass.simpleName}")
    false
}

/**
 * React to an event: through a timeline that has it (local echo), else a
 * direct send. A thread view shows replies from the room's timeline its own
 * may not have.
 */
private suspend fun react(model: TimelineModel, roomId: String, eventId: String, key: String) {
    val tries = listOfNotNull(Matrix.roomTimeline(roomId), model).distinct()
    for (t in tries) {
        if (t.messages.value.none { it.id == eventId }) continue
        try {
            t.react(eventId, key)
            return
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            org.fluxchat.companion.Diag.error("Reacting $key to $eventId through the timeline", e)
        }
    }
    if (!Matrix.sendReaction(roomId, eventId, key)) org.fluxchat.companion.Diag.error("Couldn't react $key to $eventId")
}

/**
 * Answer an approval / picker: the choice's reaction, once. Sent straight to
 * the server rather than toggled through the timeline: a toggle's second tap
 * takes the answer back, its failure only shows up later in the send queue,
 * and an approval shouldn't wait in that queue behind a message that's stuck.
 * Until it's through the buttons take no taps ([Answers]); if it fails they
 * come back, with a toast.
 */
private fun answerPrompt(context: android.content.Context, roomId: String, promptId: String, key: String) {
    if (!Answers.begin(promptId, key, System.currentTimeMillis())) return
    val app = context.applicationContext
    Matrix.launch {
        if (Matrix.sendReaction(roomId, promptId, key)) {
            Answers.delivered(promptId, System.currentTimeMillis())
        } else {
            Answers.failed(promptId)
            say(app, "Couldn't send your answer. Check your connection and tap it again.")
        }
    }
}

/** Answer a question with one of its choices: the text, once (see [questionAnswer]). */
private fun answerQuestion(context: android.content.Context, model: TimelineModel?, roomId: String, root: String?, questionId: String, text: String) {
    if (!Answers.begin(questionId, text, System.currentTimeMillis())) return
    val app = context.applicationContext
    Matrix.launch {
        if (send(model, roomId, root, text)) {
            Answers.queued(questionId, System.currentTimeMillis())
        } else {
            Answers.failed(questionId)
            say(app, "Couldn't send your answer. Check your connection and tap it again.")
        }
    }
}

/** Sends being retried / deleted right now, by message: a second tap on the button waits. */
private val sendActionsInFlight: MutableSet<String> = java.util.Collections.synchronizedSet(HashSet())

/**
 * Retry, delete or cancel a message that hasn't gone out ([id]: `~` + its
 * transaction ID), through a timeline that holds its local echo — the view's
 * own, or the project's (a session shows replies from both).
 */
private suspend fun sendAction(context: android.content.Context, model: TimelineModel?, roomId: String, id: String, action: SendAction) {
    if (!id.startsWith("~") || !sendActionsInFlight.add(id)) return
    try {
        val txn = id.removePrefix("~")
        val timelines = listOfNotNull(model, Matrix.roomTimeline(roomId)).distinct()
        var done = false
        var error: Exception? = null
        for (t in timelines) {
            try {
                done = if (action == SendAction.Retry) t.retrySend(txn) else t.abortSend(txn)
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Exception) {
                error = e
            }
            if (done) break
        }
        when {
            // Un-parked, or it never was parked (the connection): either way
            // the project's queue has to be on for it to go.
            action == SendAction.Retry && (done || error == null) -> org.fluxchat.companion.matrix.SendQueue.resume(roomId)
            done -> {}
            error != null -> {
                org.fluxchat.companion.Diag.error("${action.name} on $id in $roomId", error)
                say(context, if (action == SendAction.Retry) "Couldn't try that again" else "Couldn't take that message back")
            }
            // Nothing left to abort, and it's gone from the list: it was removed meanwhile.
            timelines.none { t -> t.messages.value.any { it.id == id } } -> {}
            else -> say(context, "Too late: that message has been sent")
        }
    } finally {
        sendActionsInFlight.remove(id)
    }
}

/** Above the composer: what's "sending" here waits for a failed send that's on another screen. */
@Composable
private fun HeldUpCard(inSession: Boolean, onShow: () -> Unit) {
    Surface(
        color = MaterialTheme.colorScheme.errorContainer,
        contentColor = MaterialTheme.colorScheme.onErrorContainer,
        shape = RoundedCornerShape(16.dp),
        modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp),
    ) {
        Row(Modifier.padding(start = 14.dp, end = 4.dp, top = 4.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(
                if (inSession) "A message in another session wasn't sent. What you send here waits for it."
                else "An earlier message in this project wasn't sent. What you send here waits for it.",
                style = MaterialTheme.typography.bodySmall, modifier = Modifier.weight(1f),
            )
            TextButton(onClick = onShow) { Text("Show") }
        }
    }
}

@Composable
internal fun RunCard(run: RunInfo, now: Long, canStop: Boolean, onStop: () -> Unit, onOpen: (() -> Unit)?, label: String? = null) {
    Surface(
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        shape = RoundedCornerShape(24.dp),
        modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp),
        onClick = onOpen ?: {},
        enabled = onOpen != null,
    ) {
        Column(Modifier.padding(horizontal = 16.dp, vertical = 12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    label?.let { Text("In a session: $it", style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis) }
                    StatePill(run.state, buildList {
                        add(run.state.label)
                        if (run.state.live) add(elapsed(run.since, now))
                        run.costUsd?.let { add("$" + String.format(java.util.Locale.US, "%.2f", it)) }
                    }.joinToString(" · "))
                    Runs.stepLabel(run.step)?.takeIf { run.state.live }?.let {
                        Text(
                            if (run.steps > 1) "$it  (step ${run.steps})" else it,
                            style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 2, overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
                if (canStop) {
                    FilledTonalButton(onClick = onStop) {
                        Icon(Icons.Default.Stop, null, Modifier.size(18.dp))
                        Spacer(Modifier.width(6.dp))
                        Text("Stop")
                    }
                } else if (onOpen != null) {
                    Icon(Icons.AutoMirrored.Outlined.OpenInNew, "Open", tint = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            when {
                run.progress != null -> LinearProgressIndicator(progress = { run.progress }, modifier = Modifier.fillMaxWidth(), color = run.state.color())
                run.state == RunState.Working -> LinearProgressIndicator(Modifier.fillMaxWidth(), color = run.state.color())
                else -> {}
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun Composer(
    harness: Harness,
    working: Boolean,
    placeholder: String? = null,
    canAttach: Boolean = false,
    onAttach: (Attachment, String) -> Unit = { _, _ -> },
    /** The room whose composer this is: it takes what was shared to it. */
    shareFor: String? = null,
    /** Whether there's something in the composer (text, an attachment, a shot being taken). */
    onDrafting: (Boolean) -> Unit = {},
    /** Something sent from here that nothing would take ([Unsent]): it comes back, [onUnsentTaken] once it has. */
    unsent: Unsent.Item? = null,
    onUnsentTaken: (Unsent.Item) -> Unit = {},
    onSend: (String, BusyMode) -> Unit,
) {
    var text by rememberSaveable { mutableStateOf("") }
    var mode by rememberSaveable { mutableStateOf(BusyMode.Send) }
    // Saved with the screen: turning the phone (or coming back from the
    // camera in the other orientation) recreates the activity, and what was
    // attached — or the shot just taken — was gone.
    var attachment by rememberSaveable(stateSaver = AttachmentSaver) { mutableStateOf<Attachment?>(null) }
    var attachMenu by remember { mutableStateOf(false) }
    val context = LocalContext.current
    val app = context.applicationContext
    // Put [a] on the composer; what it replaces is done with (a camera shot's staged file goes).
    val attach: (Attachment?) -> Unit = { a ->
        val old = attachment
        attachment = a
        if (old != null && old.uri != a?.uri) Matrix.launch { Uploads.release(app, old) }
    }
    val picked: (android.net.Uri?) -> Unit = { uri -> if (uri != null) attach(Uploads.describe(context, uri)) }
    val pickMedia = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia(), picked)
    val pickFile = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument(), picked)
    // Direct capture: the system camera writes into a staging file we hand it
    // through the FileProvider (no CAMERA permission — their camera, not ours);
    // a cancelled shot leaves nothing behind. The shot in progress is saved
    // state too (as text): its result arrives in a recreated activity as
    // often as not, and found nothing to put it in.
    var shotUri by rememberSaveable { mutableStateOf<String?>(null) }
    var shotPath by rememberSaveable { mutableStateOf<String?>(null) }
    var shotTaken by rememberSaveable { mutableStateOf(false) }
    val finishShot: (Boolean) -> Unit = { ok ->
        val f = shotPath?.let { java.io.File(it) }
        if (ok && shotUri != null && f != null && f.length() > 0) {
            shotTaken = true
        } else {
            shotUri = null
            shotPath = null
            f?.parentFile?.deleteRecursively()
        }
    }
    // A shot that's been taken: upright first (the agents read pixels, not
    // EXIF), then onto the composer. An effect of the saved state rather than
    // a thread started by the result: recreated half-way, it runs again.
    LaunchedEffect(shotTaken, shotPath) {
        val uri = shotUri
        val path = shotPath
        if (!shotTaken || uri == null || path == null) return@LaunchedEffect
        val a = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
            Uploads.normalizeImage(app, java.io.File(path))
            Uploads.describe(app, android.net.Uri.parse(uri))
        }
        attach(a)
        shotUri = null
        shotPath = null
        shotTaken = false
    }
    val takePhoto = rememberLauncherForActivityResult(ActivityResultContracts.TakePicture(), finishShot)
    val takeVideo = rememberLauncherForActivityResult(ActivityResultContracts.CaptureVideo(), finishShot)
    val startShot: (Boolean) -> Unit = { video ->
        val f = Uploads.stagingFile(context, Uploads.shotName(video, java.time.LocalDateTime.now()))
        val uri = androidx.core.content.FileProvider.getUriForFile(context, "${context.packageName}.files", f)
        shotUri = uri.toString()
        shotPath = f.path
        shotTaken = false
        try {
            if (video) takeVideo.launchHopping(uri) else takePhoto.launchHopping(uri)
        } catch (e: Exception) {
            shotUri = null
            shotPath = null
            f.parentFile?.deleteRecursively()
            org.fluxchat.companion.Diag.error("Opening the camera", e)
            android.widget.Toast.makeText(context, "No camera app on this phone", android.widget.Toast.LENGTH_SHORT).show()
        }
    }
    val modes = BusyMode.available(harness)
    val dictate = rememberDictation { said -> text = withDictation(text, said) }
    val shared by ShareInbox.pending.collectAsState()
    LaunchedEffect(shareFor, shared) {
        if (shareFor != null && shared != null) ShareInbox.take(shareFor)?.let { s ->
            s.text?.let { t -> text = if (text.isBlank()) t else text + "\n" + t }
            s.attachment?.let { attach(it) }
        }
    }
    // An attachment that came back with the saved state may not be readable
    // any more (a picked file's grant ends with the process): say so now, not
    // as a failed send later.
    LaunchedEffect(attachment?.uri) {
        val a = attachment ?: return@LaunchedEffect
        val readable = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
            try {
                app.contentResolver.openAssetFileDescriptor(a.uri, "r")?.close() != null
            } catch (e: Exception) {
                org.fluxchat.companion.Diag.error("The attachment ${a.name} can't be read any more", e)
                false
            }
        }
        if (!readable && attachment?.uri == a.uri) {
            attach(null)
            android.widget.Toast.makeText(context, "${a.name} is no longer available. Attach it again.", android.widget.Toast.LENGTH_LONG).show()
        }
    }
    val drafting = text.isNotBlank() || attachment != null || shotUri != null
    val reportDrafting by rememberUpdatedState(onDrafting)
    LaunchedEffect(drafting) { reportDrafting(drafting) }
    // A failed hand-over comes back into an empty composer by itself;
    // into a busy one it waits on a card (below) rather than being mixed in.
    val takeUnsent by rememberUpdatedState(onUnsentTaken)
    LaunchedEffect(unsent, drafting) {
        if (unsent != null && !drafting) {
            text = unsent.text
            mode = unsent.mode
            attach(unsent.attachment)
            takeUnsent(unsent)
        }
    }
    Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        if (unsent != null && drafting) {
            UnsentCard(
                unsent,
                onSendAgain = {
                    onUnsentTaken(unsent)
                    if (unsent.attachment != null) onAttach(unsent.attachment, unsent.text) else onSend(unsent.text, unsent.mode)
                },
                onDiscard = {
                    onUnsentTaken(unsent)
                    unsent.attachment?.let { a -> Matrix.launch { Uploads.release(app, a) } }
                },
            )
        }
        if (shotTaken) {
            Text("Getting your shot ready…", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(start = 8.dp))
        }
        attachment?.let { a -> AttachmentCard(a) { attach(null) } }
        // Agent commands while typing "/name".
        if (text.startsWith("/") && !text.startsWith("//") && ' ' !in text) {
            val prefix = text.removePrefix("/")
            val matches = harness.commands.filter { it.name.startsWith(prefix) }.take(12)
            if (matches.isNotEmpty()) {
                LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(matches) { c ->
                        SuggestionChip(onClick = { text = "/${c.name} " }, label = { Text("/${c.name}") })
                    }
                }
            }
        }
        if (working && modes.size > 1) {
            SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
                modes.forEachIndexed { i, m ->
                    SegmentedButton(
                        selected = mode == m,
                        onClick = { mode = m },
                        shape = SegmentedButtonDefaults.itemShape(i, modes.size),
                    ) { Text(m.label) }
                }
            }
        }
        Row(verticalAlignment = Alignment.Bottom) {
            if (canAttach) {
                Box {
                    IconButton(onClick = { attachMenu = true }, modifier = Modifier.size(56.dp)) { Icon(Icons.Outlined.AttachFile, "Attach") }
                    DropdownMenu(expanded = attachMenu && unlocked(), onDismissRequest = { attachMenu = false }, shape = MaterialTheme.shapes.medium) {
                        DropdownMenuItem(
                            text = { Text("Take photo") },
                            leadingIcon = { Icon(Icons.Outlined.PhotoCamera, null) },
                            onClick = { attachMenu = false; startShot(false) },
                        )
                        DropdownMenuItem(
                            text = { Text("Take video") },
                            leadingIcon = { Icon(Icons.Outlined.Videocam, null) },
                            onClick = { attachMenu = false; startShot(true) },
                        )
                        DropdownMenuItem(
                            text = { Text("Photo or video") },
                            leadingIcon = { Icon(Icons.Outlined.Image, null) },
                            onClick = {
                                attachMenu = false
                                pickMedia.launchHopping(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageAndVideo))
                            },
                        )
                        DropdownMenuItem(
                            text = { Text("File") },
                            leadingIcon = { Icon(Icons.Outlined.Description, null) },
                            onClick = { attachMenu = false; pickFile.launchHopping(arrayOf("*/*")) },
                        )
                    }
                }
            }
            TextField(
                value = text,
                onValueChange = { text = it },
                modifier = Modifier.weight(1f),
                placeholder = {
                    Text(
                        when {
                            attachment != null -> "Add a caption"
                            working -> "Message the working agent"
                            else -> placeholder ?: "Message"
                        },
                    )
                },
                maxLines = 6,
                trailingIcon = dictate?.let { d -> { IconButton(onClick = d) { Icon(Icons.Outlined.Mic, "Speak") } } },
                shape = RoundedCornerShape(28.dp),
                colors = TextFieldDefaults.colors(
                    focusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                    unfocusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                    focusedIndicatorColor = Color.Transparent,
                    unfocusedIndicatorColor = Color.Transparent,
                    disabledIndicatorColor = Color.Transparent,
                ),
            )
            Spacer(Modifier.width(8.dp))
            FilledIconButton(
                onClick = {
                    val t = text.trim()
                    val a = attachment
                    if (a != null) {
                        onAttach(a, t)
                        attachment = null
                        text = ""
                    } else if (t.isNotEmpty()) {
                        onSend(t, if (working) mode else BusyMode.Send)
                        text = ""
                    }
                },
                enabled = text.isNotBlank() || attachment != null,
                modifier = Modifier.size(56.dp),
                colors = IconButtonDefaults.filledIconButtonColors(),
            ) { Icon(Icons.AutoMirrored.Filled.Send, "Send") }
        }
    }
}

/** The composer's pending attachment across the activity being recreated (see [org.fluxchat.companion.matrix.SavedAttachment]). */
private val AttachmentSaver = androidx.compose.runtime.saveable.listSaver<Attachment?, String>(
    save = { a -> a?.let { Uploads.saved(it) } ?: emptyList() },
    restore = { Uploads.restored(it) },
)

/** Something sent from here that couldn't be handed over, while the composer has other things in it. */
@Composable
private fun UnsentCard(item: Unsent.Item, onSendAgain: () -> Unit, onDiscard: () -> Unit) {
    Surface(
        color = MaterialTheme.colorScheme.errorContainer,
        contentColor = MaterialTheme.colorScheme.onErrorContainer,
        shape = RoundedCornerShape(16.dp),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(Modifier.padding(start = 14.dp, end = 4.dp, top = 4.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text("Not sent", style = MaterialTheme.typography.labelMedium)
                Text(
                    listOfNotNull(item.attachment?.name, item.text.takeIf { it.isNotBlank() }).joinToString(" · "),
                    style = MaterialTheme.typography.bodySmall, maxLines = 2, overflow = TextOverflow.Ellipsis,
                )
            }
            TextButton(onClick = onSendAgain) { Text("Send again") }
            IconButton(onClick = onDiscard) { Icon(Icons.Outlined.Close, "Discard") }
        }
    }
}

/** The file waiting to go with the next send: a preview, its name and size, and remove. */
@Composable
private fun AttachmentCard(a: Attachment, onRemove: () -> Unit) {
    val context = LocalContext.current
    val preview by produceState<androidx.compose.ui.graphics.ImageBitmap?>(null, a.uri) {
        if (a.isImage || a.isVideo) value = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
            runCatching {
                if (android.os.Build.VERSION.SDK_INT >= 29) {
                    context.contentResolver.loadThumbnail(a.uri, android.util.Size(PREVIEW_PX, PREVIEW_PX), null).asImageBitmap()
                } else if (a.isImage) {
                    // Android 9: decode a downsampled copy.
                    val bounds = android.graphics.BitmapFactory.Options().apply { inJustDecodeBounds = true }
                    context.contentResolver.openInputStream(a.uri)?.use { android.graphics.BitmapFactory.decodeStream(it, null, bounds) }
                    var sample = 1
                    while (bounds.outWidth / (sample * 2) >= PREVIEW_PX && bounds.outHeight / (sample * 2) >= PREVIEW_PX) sample *= 2
                    val opts = android.graphics.BitmapFactory.Options().apply { inSampleSize = sample }
                    context.contentResolver.openInputStream(a.uri)?.use { android.graphics.BitmapFactory.decodeStream(it, null, opts) }?.asImageBitmap()
                } else null
            }.getOrNull()
        }
    }
    Surface(color = MaterialTheme.colorScheme.surfaceContainerHigh, shape = RoundedCornerShape(16.dp), modifier = Modifier.fillMaxWidth()) {
        Row(Modifier.padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(
                Modifier.size(48.dp).clip(RoundedCornerShape(10.dp)).background(MaterialTheme.colorScheme.surfaceContainerHighest),
                contentAlignment = Alignment.Center,
            ) {
                val p = preview
                if (p != null) {
                    Image(p, null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
                } else {
                    Icon(if (a.isImage) Icons.Outlined.Image else Icons.Outlined.Description, null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text(a.name, style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(
                    listOfNotNull(a.size?.let { android.text.format.Formatter.formatShortFileSize(context, it) }, a.mime).joinToString(" · "),
                    style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1, overflow = TextOverflow.Ellipsis,
                )
            }
            IconButton(onClick = onRemove) { Icon(Icons.Outlined.Close, "Remove attachment") }
        }
    }
}

/** Size of the attachment preview's thumbnail, in pixels. */
private const val PREVIEW_PX = 192
