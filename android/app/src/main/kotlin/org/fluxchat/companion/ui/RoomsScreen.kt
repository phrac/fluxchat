package org.fluxchat.companion.ui

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowForward
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.Forum
import androidx.compose.material.icons.outlined.History
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.outlined.SmartToy
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Badge
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.AssistChip
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.fluxchat.companion.agents.RunInfo
import org.fluxchat.companion.agents.RunState
import org.fluxchat.companion.agents.Runs
import org.fluxchat.companion.agents.ViewKey
import org.fluxchat.companion.matrix.AgentRoom
import org.fluxchat.companion.matrix.Matrix
import org.fluxchat.companion.matrix.PinStore

/** Agent rooms: pinned projects first, then waiting on you, then running, then by last activity. */
internal fun agentRoomOrder(pins: PinStore.Pins): Comparator<AgentRoom> =
    compareBy<AgentRoom> { if (pins.hasRoom(it.roomId)) 0 else 1 }
        .thenBy { r -> r.headline?.second?.takeIf { it.state.live }?.rank()?.first ?: 9 }
        .thenByDescending { it.last?.ts ?: 0L }

/** The standing order, reading the pins as they are now (for non-composable callers). */
internal val AGENT_ROOM_ORDER: Comparator<AgentRoom>
    get() = agentRoomOrder(Matrix.pins.value)

private enum class RoomFilter(val label: String) { All("All"), NeedsYou("Needs you"), Running("Running") }

/** A finished run keeps its row on the Projects screen this long, so its
 * `result` is readable before the row folds into history. */
private const val FINISHED_ROW_WINDOW_MS = 10 * 60 * 1000L

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun RoomsScreen(go: (Screen) -> Unit) {
    val rooms by Matrix.agentRooms.collectAsState()
    val list by Matrix.agentList.collectAsState()
    val update by org.fluxchat.companion.Update.available.collectAsState()
    val context = androidx.compose.ui.platform.LocalContext.current
    val pins by Matrix.pins.collectAsState()
    val now by rememberNow()
    val scope = rememberCoroutineScope()
    var refreshing by rememberSaveable { mutableStateOf(false) }
    var filter by rememberSaveable { mutableStateOf(RoomFilter.All) }
    var creating by rememberSaveable { mutableStateOf(false) }
    val scroll = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()

    val allRuns = rooms.values.flatMap { r -> r.runs.values.filterNotNull() }
    val running = allRuns.count { it.state == RunState.Working }
    val waiting = allRuns.count { it.state == RunState.NeedsYou }

    Scaffold(
        modifier = Modifier.nestedScroll(scroll.nestedScrollConnection),
        topBar = {
            LargeTopAppBar(
                title = { Text("Projects") },
                actions = {
                    update?.let {
                        // Subtle: a plain assist chip, no badge or colour shout.
                        AssistChip(onClick = { org.fluxchat.companion.Update.openRelease(context) }, label = { Text("Update available") })
                    }
                    IconButton(onClick = { creating = true }) { Icon(Icons.Outlined.Add, "New project") }
                    IconButton(onClick = { go(Screen.Search) }) { Icon(Icons.Outlined.Search, "Search sessions") }
                    IconButton(onClick = { go(Screen.History) }) { Icon(Icons.Outlined.History, "Run history") }
                    IconButton(onClick = { go(Screen.Agents) }) { Icon(Icons.Outlined.SmartToy, "Your agents") }
                    IconButton(onClick = { go(Screen.Settings) }) { Icon(Icons.Outlined.Settings, "Settings") }
                },
                scrollBehavior = scroll,
            )
        },
    ) { pad ->
        PullToRefreshBox(
            isRefreshing = refreshing,
            onRefresh = {
                refreshing = true
                scope.launch {
                    Matrix.resumed()
                    Matrix.loadAgentList()
                    org.fluxchat.companion.Update.check(force = true)
                    delay(800)
                    refreshing = false
                }
            },
            modifier = Modifier.fillMaxSize().padding(top = pad.calculateTopPadding()),
        ) {
            when {
                list.agents.isEmpty() -> Empty(
                    Icons.Outlined.SmartToy,
                    "No agents yet",
                    "Mark your agents in desktop fluxchat (their profile card → AI agent) and they show up here. Or add one now.",
                    "Add an agent",
                ) { go(Screen.Agents) }
                rooms.isEmpty() -> Empty(
                    Icons.Outlined.Forum,
                    "No projects yet",
                    "Start one and your agents join right away. Or projects appear here once one of your agents joins one.",
                    "New project",
                ) { creating = true }
                else -> {
                    val shown = rooms.values.filter { r ->
                        when (filter) {
                            RoomFilter.All -> true
                            RoomFilter.NeedsYou -> r.runs.values.any { it?.state == RunState.NeedsYou }
                            RoomFilter.Running -> r.runs.values.any { it?.state == RunState.Working }
                        }
                    }.sortedWith(agentRoomOrder(pins))
                    // Rooms re-sort as they load and as runs start: stay at the top while
                    // you're there (Compose would follow the room that was first).
                    val listState = androidx.compose.foundation.lazy.rememberLazyListState()
                    FollowNewest(listState, shown.map { it.roomId })
                    LazyColumn(
                        state = listState,
                        contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 4.dp, bottom = pad.calculateBottomPadding() + 24.dp),
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                        modifier = Modifier.fillMaxSize(),
                    ) {
                        item {
                            LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                items(RoomFilter.entries) { f ->
                                    val count = when (f) {
                                        RoomFilter.All -> null
                                        RoomFilter.NeedsYou -> waiting
                                        RoomFilter.Running -> running
                                    }
                                    FilterChip(
                                        selected = filter == f,
                                        onClick = { filter = f },
                                        label = { Text(if (count != null && count > 0) "${f.label} · $count" else f.label) },
                                        leadingIcon = when (f) {
                                            RoomFilter.NeedsYou -> ({ StateDot(RunState.NeedsYou, 8.dp) })
                                            RoomFilter.Running -> ({ StateDot(RunState.Working, 8.dp) })
                                            RoomFilter.All -> null
                                        },
                                    )
                                }
                            }
                        }
                        if (shown.isEmpty()) {
                            item {
                                Text(
                                    if (filter == RoomFilter.NeedsYou) "Nothing waiting on you." else "Nothing running right now.",
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    modifier = Modifier.fillMaxWidth().padding(32.dp),
                                    textAlign = TextAlign.Center,
                                )
                            }
                        }
                        items(shown, key = { it.roomId }) { room -> RoomCard(room, now, go, Modifier.animateItem()) }
                    }
                }
            }
        }
    }

    if (creating) {
        CreateProjectFlow(list = list, go = go, onCreated = { creating = false }, onDismiss = { Matrix.clearError(); creating = false })
    }
}

/**
 * The dialog + the one thing it does: create, then straight into the new
 * project. [create] is the Matrix call; tests inject a fake to watch it.
 */
@Composable
internal fun CreateProjectFlow(
    list: org.fluxchat.companion.matrix.AgentList,
    go: (Screen) -> Unit,
    onCreated: () -> Unit,
    onDismiss: () -> Unit,
    create: suspend (String, org.fluxchat.companion.matrix.AgentList) -> String? = Matrix::createProject,
) {
    val scope = rememberCoroutineScope()
    NewProjectDialog(agents = list.agents.size, onDismiss = onDismiss) { name ->
        scope.launch {
            val roomId = create(name, list)
            if (roomId != null) {
                onCreated()
                // Straight in: the first message can be sent immediately.
                go(Screen.Room(roomId))
            }
        }
    }
}

/** Name a new project; the app invites every agent (the list's count is the promise). */
@Composable
private fun NewProjectDialog(agents: Int, onDismiss: () -> Unit, onCreate: (String) -> Unit) {
    var name by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    val error by Matrix.error.collectAsState()
    AlertDialog(
        onDismissRequest = { if (!busy) onDismiss() },
        title = { Text("New project") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    name, { name = it }, singleLine = true, modifier = Modifier.fillMaxWidth(),
                    label = { Text("Project name") }, placeholder = { Text("Weekend build") },
                )
                Text(
                    if (agents > 0) "Your $agents ${if (agents == 1) "agent" else "agents"} ${if (agents == 1) "is" else "are"} invited and can start right away."
                    else "Add agents first and they'll be invited to every new project.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                error?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium) }
            }
        },
        confirmButton = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (busy) CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                TextButton(onClick = { busy = true; onCreate(name.trim()) }, enabled = name.isNotBlank() && !busy) { Text("Create") }
            }
        },
        dismissButton = { TextButton(onClick = onDismiss, enabled = !busy) { Text("Cancel") } },
    )
}

@Composable
private fun Empty(icon: ImageVector, title: String, body: String, action: String?, onAction: () -> Unit) {
    Box(Modifier.fillMaxSize().padding(32.dp), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Box(Modifier.size(88.dp).background(MaterialTheme.colorScheme.secondaryContainer, CircleShape), contentAlignment = Alignment.Center) {
                Icon(icon, null, Modifier.size(40.dp), tint = MaterialTheme.colorScheme.onSecondaryContainer)
            }
            Text(title, style = MaterialTheme.typography.titleLarge, textAlign = TextAlign.Center)
            Text(body, color = MaterialTheme.colorScheme.onSurfaceVariant, textAlign = TextAlign.Center)
            action?.let { Button(onClick = onAction) { Text(it) } }
        }
    }
}

@Composable
internal fun RoomCard(room: AgentRoom, now: Long, go: (Screen) -> Unit, modifier: Modifier, msgs: List<org.fluxchat.companion.agents.Msg> = Matrix.roomTimeline(room.roomId)?.messages?.value.orEmpty()) {
    val headline = room.headline
    val roomRun = headline?.takeIf { it.first.threadRoot == null }?.second
    val threadRuns: List<Pair<ViewKey, RunInfo>> = room.runs
        .mapNotNull { (k, r) ->
            // Live runs, plus recently finished ones: their `result` is the row's body.
            if (k.threadRoot != null && r != null &&
                (r.state.live || now - r.since < FINISHED_ROW_WINDOW_MS)
            ) k to r else null
        }
        .sortedWith(compareBy({ it.second.rank().first }, { it.second.rank().second }))
    val needsYou = room.runs.values.any { it?.state == RunState.NeedsYou }
    Card(
        onClick = { go(Screen.Room(room.roomId)) },
        modifier = modifier.fillMaxWidth().animateContentSize(),
        shape = MaterialTheme.shapes.large,
        colors = CardDefaults.cardColors(
            containerColor = if (needsYou) LocalStatus.current.needsYouContainer else MaterialTheme.colorScheme.surfaceContainerLow,
        ),
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Avatar(room.roomId, room.name, room.avatar, 44.dp, room = !room.isDm, state = headline?.second?.state?.takeIf { it.live })
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(room.name, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                        room.last?.let { Text(elapsed(it.ts, now), style = MaterialTheme.typography.labelMedium, color = if (room.unread > 0) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant) }
                    }
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        val typing = typingNames(room.roomId, msgs)
                        val m = room.last
                        val preview = when {
                            typing != null -> typing
                            m != null -> "${shortName(m.sender, m.senderName)}: " +
                                (Runs.progressSteps(m.body)?.last() ?: m.body.lineSequence().firstOrNull { it.isNotBlank() } ?: m.file?.name ?: if (m.media != null) "Image" else "")
                            else -> ""
                        }
                        Text(
                            preview,
                            style = MaterialTheme.typography.bodyMedium,
                            color = if (typing != null) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 2,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f),
                        )
                        if (room.unread > 0) {
                            Spacer(Modifier.width(8.dp))
                            Badge(containerColor = MaterialTheme.colorScheme.primary) { Text(if (room.unread > 99) "99+" else room.unread.toString()) }
                        }
                    }
                }
            }
            if (roomRun != null && (roomRun.state.live || now - roomRun.since < 10 * 60 * 1000)) {
                StatePill(roomRun.state, runLine(roomRun, now))
                // The room's run is over: its one-line outcome is the row's body.
                runResult(roomRun)?.let {
                    Text(it, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 2, overflow = TextOverflow.Ellipsis)
                }
            }
            if (threadRuns.isNotEmpty()) {
                Column(
                    Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.surfaceContainerHighest.copy(alpha = 0.5f), RoundedCornerShape(16.dp)),
                ) {
                    threadRuns.take(4).forEachIndexed { i, (key, run) ->
                        if (i > 0) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f), modifier = Modifier.padding(horizontal = 12.dp))
                        val root = msgs.firstOrNull { it.id == key.threadRoot }
                        val replies = msgs.filter { it.threadRoot == key.threadRoot }
                        Row(
                            Modifier.fillMaxWidth().clickable { go(Screen.Thread(room.roomId, key.threadRoot!!)) }.padding(horizontal = 12.dp, vertical = 10.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            StateDot(run.state, 10.dp)
                            Spacer(Modifier.width(10.dp))
                            Column(Modifier.weight(1f)) {
                                Text(threadTitle(root, replies), style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                Text(runLine(run, now), style = MaterialTheme.typography.labelMedium, color = run.state.color(), maxLines = 1, overflow = TextOverflow.Ellipsis)
                                // A finished run's body: how it ended (protocol `result`).
                                runResult(run)?.let {
                                    Text(it, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 2, overflow = TextOverflow.Ellipsis)
                                }
                            }
                            Icon(Icons.AutoMirrored.Outlined.ArrowForward, null, Modifier.size(18.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                }
            }
        }
    }
}

/** "Hermes is typing…" for a room, or null. */
@Composable
fun typingNames(roomId: String, msgs: List<org.fluxchat.companion.agents.Msg>): String? {
    val typing by Matrix.typing.collectAsState()
    val ids = typing[roomId].orEmpty()
    if (ids.isEmpty()) return null
    val names = ids.map { id -> shortName(id, msgs.lastOrNull { it.sender == id }?.senderName) }
    return when (names.size) {
        1 -> "${names[0]} is typing…"
        else -> "${names.joinToString()} are typing…"
    }
}
