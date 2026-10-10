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
import androidx.compose.material.icons.filled.Stop
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
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import org.fluxchat.companion.agents.BusyMode
import org.fluxchat.companion.agents.Harness
import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.agents.RunInfo
import org.fluxchat.companion.agents.RunState
import org.fluxchat.companion.agents.Runs
import org.fluxchat.companion.agents.ViewKey
import org.fluxchat.companion.matrix.AgentList
import org.fluxchat.companion.matrix.Matrix
import org.fluxchat.companion.matrix.TimelineModel

/** How long after you send a request its thread opens by itself when the agent starts on it. */
private const val FOLLOW_WINDOW_MS = 3 * 60 * 1000L

/** A room's timeline, or a thread's (`root`): messages, the run, the composer. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ChatScreen(roomId: String, root: String?, go: (Screen) -> Unit, back: () -> Unit, swap: (Screen) -> Unit = go) {
    val shared = remember(roomId, root) { if (root == null) Matrix.roomTimeline(roomId) else null }
    // Threads (and rooms that aren't kept open) get their own timeline while on screen.
    val own by produceState<TimelineModel?>(null, roomId, root) {
        if (shared == null) value = if (root != null) Matrix.openThread(roomId, root) else Matrix.openLive(roomId)
    }
    // Close the timeline this effect was for (not whatever `own` is by then).
    val opened = own
    DisposableEffect(opened) { onDispose { opened?.close() } }
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
        else Runs.roomRuns(roomId, msgs, agents, list::harnessOf, Matrix.isDm(roomId), now).runs[key] ?: agentRoom?.runs?.get(key)
    }
    val agent = run?.agent?.takeIf { it.isNotEmpty() } ?: agents.firstOrNull()
    val harness = list.harnessOf(agent ?: "")
    // A room where every request gets its own thread reads as its sessions,
    // a card each; the plain timeline is a tap away.
    val threaded = root == null && agent != null && !Matrix.isDm(roomId) && Runs.repliesInThreads(msgs, agent, harness, false)
    var timelineView by rememberSaveable(roomId) { mutableStateOf(false) }
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
    LaunchedEffect(agentRoom?.runs) {
        if (root != null || sentAt == 0L) return@LaunchedEffect
        val target = agentRoom?.runs?.entries?.firstOrNull { (k, r) ->
            k.threadRoot != null && r != null && r.state.live && r.since >= sentAt - 5_000 && k.threadRoot !in followed
        }?.key?.threadRoot
        if (target != null && System.currentTimeMillis() - sentAt < FOLLOW_WINDOW_MS) {
            followed = followed + target
            go(Screen.Thread(roomId, target))
        }
    }

    LaunchedEffect(msgs.size) { model?.markRead() }

    val title = if (root == null) Matrix.roomName(roomId) else threadTitle(msgs.firstOrNull { it.id == root }, msgs.filter { it.id != root })
    var menu by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    val sendRaw: (String) -> Unit = { text -> scope.launch { send(model, roomId, root, text) } }
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
                    if (threaded) {
                        IconButton(onClick = { timelineView = !timelineView }) {
                            if (cards) Icon(Icons.AutoMirrored.Outlined.Chat, "Show the timeline")
                            else Icon(Icons.Outlined.ViewAgenda, "Show sessions")
                        }
                    }
                    if (sessionsButton) {
                        IconButton(onClick = { scope.launch { drawer.open() } }) { Icon(Icons.Outlined.History, "Sessions") }
                    }
                    if (harness.menu.isNotEmpty()) {
                        Box {
                            IconButton(onClick = { menu = true }) { Icon(Icons.Default.MoreVert, "Agent commands") }
                            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }, shape = MaterialTheme.shapes.medium) {
                                for (item in harness.menu) {
                                    DropdownMenuItem(text = { Text(item.label) }, onClick = { menu = false; command(item.command) })
                                }
                            }
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
                    SessionFeed(roomId, all, agents, agentRoom?.runs.orEmpty(), sessionList, now) { r -> go(Screen.Thread(roomId, r)) }
                } else if (model == null) {
                    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
                } else {
                    // A session (a thread, or a DM with the agent) reads as a transcript.
                    val transcript = agents.isNotEmpty() && (root != null || Matrix.isDm(roomId))
                    MessageList(model, msgs, root, agents, list, now, go, roomId, transcript, run)
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
                    model?.let { m -> scope.launch { sendAttachment(context, m, a, caption) } }
                },
                onSend = { text, mode ->
                    val out = Runs.outgoing(harness, text, run?.state == RunState.Working, mode, agent?.takeIf { !Matrix.isDm(roomId) })
                    if (root == null) sentAt = System.currentTimeMillis()
                    org.fluxchat.companion.quick.Quick.remember(context, roomId)
                    sendRaw(out)
                },
            )
        }
    } }

    BoxWithConstraints(Modifier.fillMaxSize()) {
        val wide = maxWidth >= WIDE_LAYOUT
        when {
            !hasSessions -> content(false)
            wide -> Row(Modifier.fillMaxSize()) {
                PermanentDrawerSheet(Modifier.width(SESSIONS_WIDTH)) { SessionsPane(roomId, root, openSession, newSession) }
                Box(Modifier.weight(1f)) { content(false) }
            }
            else -> ModalNavigationDrawer(
                drawerState = drawer,
                // The left edge is Android's back gesture: the drawer opens from its button.
                gesturesEnabled = drawer.isOpen,
                drawerContent = { ModalDrawerSheet(drawerState = drawer) { SessionsPane(roomId, root, openSession, newSession) } },
            ) { content(true) }
        }
    }
}

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
) {
    val state = rememberLazyListState()
    val scope = rememberCoroutineScope()
    val atStart by model.atStart.collectAsState()
    var loading by remember { mutableStateOf(false) }
    val nearTop by remember { derivedStateOf { (state.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0) >= state.layoutInfo.totalItemsCount - 4 } }
    val awayFromBottom by remember { derivedStateOf { state.firstVisibleItemIndex > 2 } }
    LaunchedEffect(nearTop, msgs.size) {
        if (nearTop && !atStart && !loading) {
            loading = true
            model.paginateBack()
            loading = false
        }
    }
    val reversed = remember(msgs) { msgs.asReversed() }
    FollowNewest(state, msgs)
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
    @Composable
    fun transcriptRow(i: Int, prev: Msg?) {
        val m = msgs[i]
        val isAgent = m.sender in agents
        val h = list.harnessOf(m.sender)
        val prompt = if (isAgent) Runs.openPrompt(msgs, i, h, now) else emptyList()
        TranscriptRow(
            m = m,
            prev = prev,
            isAgent = isAgent,
            hiddenReactions = statusReaction,
            answered = if (isAgent && prompt.isEmpty()) Runs.answeredApproval(m, h, agents) else null,
            prompt = prompt,
            question = if (isAgent) Runs.pendingQuestion(msgs, i, m.sender, h, now) else null,
            live = m.id == liveSteps,
            onReact = { key -> scope.launch { react(model, roomId, m.id, key) } },
            onAnswer = { text -> scope.launch { send(model, roomId, root, text) } },
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
                        onReact = { key -> scope.launch { react(model, roomId, m.id, key) } },
                        onAnswer = { text -> scope.launch { send(model, roomId, root, text) } },
                        onOpenThread = if (root == null && m.hasThread) ({ go(Screen.Thread(roomId, m.id)) }) else null,
                        threadRun = if (root == null) Matrix.agentRooms.value[roomId]?.runs?.get(ViewKey(roomId, m.id)) else null,
                        now = now,
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
private suspend fun send(model: TimelineModel?, roomId: String, root: String?, text: String) {
    val viaTimeline = model?.let { m -> runCatching { m.send(text) }.onFailure { org.fluxchat.companion.Diag.error("Sending through the timeline in $roomId", it) }.isSuccess } ?: false
    if (!viaTimeline && !Matrix.sendText(roomId, root, text, inReplyTo = null)) org.fluxchat.companion.Diag.error("Couldn't send to $roomId")
}

/** Upload a picked file to the view (failures: a toast, and Diagnostics). */
private suspend fun sendAttachment(context: android.content.Context, model: TimelineModel, a: Attachment, caption: String) {
    runCatching { model.sendAttachment(context, a, caption) }.onFailure {
        org.fluxchat.companion.Diag.error("Sending ${a.name} (${a.mime}) to ${model.roomId}", it)
        kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.Main) {
            android.widget.Toast.makeText(context, "Couldn't send ${a.name}: ${it.message ?: it.javaClass.simpleName}", android.widget.Toast.LENGTH_LONG).show()
        }
    }
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
        if (runCatching { t.react(eventId, key) }.isSuccess) return
    }
    if (!Matrix.sendReaction(roomId, eventId, key)) org.fluxchat.companion.Diag.error("Couldn't react $key to $eventId")
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
                    label?.let { Text("In a thread: $it", style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis) }
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
    onSend: (String, BusyMode) -> Unit,
) {
    var text by rememberSaveable { mutableStateOf("") }
    var mode by rememberSaveable { mutableStateOf(BusyMode.Send) }
    var attachment by remember { mutableStateOf<Attachment?>(null) }
    var attachMenu by remember { mutableStateOf(false) }
    val context = LocalContext.current
    val picked: (android.net.Uri?) -> Unit = { uri -> if (uri != null) attachment = Uploads.describe(context, uri) }
    val pickMedia = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia(), picked)
    val pickFile = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument(), picked)
    val modes = BusyMode.available(harness)
    val dictate = rememberDictation { said -> text = withDictation(text, said) }
    val shared by ShareInbox.pending.collectAsState()
    LaunchedEffect(shareFor, shared) {
        if (shareFor != null && shared != null) ShareInbox.take(shareFor)?.let { s ->
            s.text?.let { t -> text = if (text.isBlank()) t else text + "\n" + t }
            s.attachment?.let { attachment = it }
        }
    }
    Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        attachment?.let { a -> AttachmentCard(a) { attachment = null } }
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
                    DropdownMenu(expanded = attachMenu, onDismissRequest = { attachMenu = false }, shape = MaterialTheme.shapes.medium) {
                        DropdownMenuItem(
                            text = { Text("Photo or video") },
                            leadingIcon = { Icon(Icons.Outlined.Image, null) },
                            onClick = {
                                attachMenu = false
                                pickMedia.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageAndVideo))
                            },
                        )
                        DropdownMenuItem(
                            text = { Text("File") },
                            leadingIcon = { Icon(Icons.Outlined.Description, null) },
                            onClick = { attachMenu = false; pickFile.launch(arrayOf("*/*")) },
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
