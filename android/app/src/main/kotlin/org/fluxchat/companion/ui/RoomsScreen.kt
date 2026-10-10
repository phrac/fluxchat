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
import androidx.compose.material.icons.outlined.Forum
import androidx.compose.material.icons.outlined.History
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.outlined.SmartToy
import androidx.compose.material3.Badge
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
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

/** Agent rooms: waiting on you, then running, then by last activity. */
internal val AGENT_ROOM_ORDER: Comparator<AgentRoom> =
    compareBy<AgentRoom> { r -> r.headline?.second?.takeIf { it.state.live }?.rank()?.first ?: 9 }
        .thenByDescending { it.last?.ts ?: 0L }

private enum class RoomFilter(val label: String) { All("All"), NeedsYou("Needs you"), Running("Running") }

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun RoomsScreen(go: (Screen) -> Unit) {
    val rooms by Matrix.agentRooms.collectAsState()
    val list by Matrix.agentList.collectAsState()
    val now by rememberNow()
    val scope = rememberCoroutineScope()
    var refreshing by rememberSaveable { mutableStateOf(false) }
    var filter by rememberSaveable { mutableStateOf(RoomFilter.All) }
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
                rooms.isEmpty() -> Empty(Icons.Outlined.Forum, "Looking for your projects", "Projects appear here once one of your agents joins them.", null) {}
                else -> {
                    val shown = rooms.values.filter { r ->
                        when (filter) {
                            RoomFilter.All -> true
                            RoomFilter.NeedsYou -> r.runs.values.any { it?.state == RunState.NeedsYou }
                            RoomFilter.Running -> r.runs.values.any { it?.state == RunState.Working }
                        }
                    }.sortedWith(AGENT_ROOM_ORDER)
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
        .mapNotNull { (k, r) -> if (k.threadRoot != null && r != null && r.state.live) k to r else null }
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
