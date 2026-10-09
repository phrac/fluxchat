package org.fluxchat.companion.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.flow.MutableStateFlow
import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.agents.RunInfo
import org.fluxchat.companion.agents.ViewKey
import org.fluxchat.companion.matrix.Matrix
import org.fluxchat.companion.matrix.Session
import org.fluxchat.companion.matrix.SessionList
import java.util.Calendar

/** When a session was last active, as the list groups them. */
private fun bucket(ts: Long, now: Long): String {
    val day = { t: Long -> Calendar.getInstance().apply { timeInMillis = t }.let { it.get(Calendar.YEAR) * 400 + it.get(Calendar.DAY_OF_YEAR) } }
    val days = day(now) - day(ts)
    return when {
        days <= 0 -> "Today"
        days == 1 -> "Yesterday"
        days < 7 -> "Previous 7 days"
        days < 30 -> "Previous 30 days"
        else -> "Older"
    }
}

/** A session's title: like a thread's (the request, unless it says nothing). */
private fun title(s: Session): String = threadTitle(
    Msg(s.root, "", "", s.rootBody, s.rootTs, isOwn = false),
    listOfNotNull(s.latest?.let { Msg("", "", "", it, s.lastTs, isOwn = false) }),
)

/**
 * A room's sessions (its threads, from the server's thread list): a drawer on
 * phones, a sidebar on wide screens, like the desktop's threads sidebar.
 */
@Composable
fun SessionsPane(roomId: String, current: String?, onOpen: (String) -> Unit, onNew: () -> Unit, modifier: Modifier = Modifier) {
    val list by produceState<SessionList?>(null, roomId) { value = Matrix.sessions(roomId) }
    val empty = remember { MutableStateFlow(emptyList<Session>()) }
    val sessions by (list?.items ?: empty).collectAsState()
    val loading by (list?.loading ?: MutableStateFlow(true)).collectAsState()
    val end by (list?.endReached ?: MutableStateFlow(false)).collectAsState()
    val rooms by Matrix.agentRooms.collectAsState()
    val runs: Map<ViewKey, RunInfo?> = rooms[roomId]?.runs.orEmpty()
    var query by rememberSaveable { mutableStateOf("") }
    val now by rememberNow(60_000)
    val state = rememberLazyListState()
    val nearEnd by remember { derivedStateOf { (state.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0) >= state.layoutInfo.totalItemsCount - 5 } }
    LaunchedEffect(nearEnd, sessions.size, query) {
        // More while scrolled to the end, or while a search has few matches loaded.
        if ((nearEnd || query.isNotBlank()) && !end && !loading) list?.more()
    }
    val shown = remember(sessions, query) {
        val q = query.trim().lowercase()
        if (q.isEmpty()) sessions
        else sessions.filter { s -> s.rootBody.lowercase().contains(q) || s.latest.orEmpty().lowercase().contains(q) }
    }

    Column(modifier.fillMaxSize().statusBarsPadding()) {
        Column(Modifier.padding(horizontal = 16.dp, vertical = 12.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text("Sessions", style = MaterialTheme.typography.titleLarge)
            Text(Matrix.roomName(roomId), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
            FilledTonalButton(onClick = onNew, modifier = Modifier.fillMaxWidth()) {
                Icon(Icons.Outlined.Add, null, Modifier.size(18.dp))
                Spacer(Modifier.width(8.dp))
                Text("New session")
            }
            TextField(
                value = query,
                onValueChange = { query = it },
                modifier = Modifier.fillMaxWidth(),
                placeholder = { Text("Search sessions") },
                leadingIcon = { Icon(Icons.Outlined.Search, null) },
                singleLine = true,
                shape = RoundedCornerShape(28.dp),
                colors = TextFieldDefaults.colors(
                    focusedIndicatorColor = Color.Transparent,
                    unfocusedIndicatorColor = Color.Transparent,
                    focusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHighest,
                    unfocusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHighest,
                ),
            )
        }
        LazyColumn(state = state, modifier = Modifier.weight(1f), contentPadding = androidx.compose.foundation.layout.PaddingValues(bottom = 24.dp)) {
            var lastBucket: String? = null
            val rows = shown.map { s -> s to bucket(s.lastTs, now) }
            for ((s, b) in rows) {
                if (b != lastBucket) {
                    lastBucket = b
                    item(key = "h:$b") {
                        Text(
                            b, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary,
                            modifier = Modifier.padding(start = 28.dp, end = 16.dp, top = 16.dp, bottom = 6.dp),
                        )
                    }
                }
                item(key = s.root) { SessionRow(s, runs[ViewKey(roomId, s.root)], s.root == current, now) { onOpen(s.root) } }
            }
            if (loading) {
                item { Box(Modifier.fillMaxWidth().padding(16.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(Modifier.size(24.dp), strokeWidth = 2.dp) } }
            } else if (shown.isEmpty()) {
                item {
                    Text(
                        when {
                            query.isNotBlank() -> "No sessions match."
                            Matrix.isDm(roomId) -> "No threads here: in a DM the agent answers in the conversation itself. Start a fresh session with New session."
                            else -> "No sessions yet. Send the agent a request to start one."
                        },
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 28.dp, vertical = 16.dp),
                    )
                }
            }
        }
    }
}

@Composable
private fun SessionRow(s: Session, run: RunInfo?, selected: Boolean, now: Long, onClick: () -> Unit) {
    val scheme = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 1.dp)
            .background(if (selected) scheme.secondaryContainer else Color.Transparent, RoundedCornerShape(28.dp))
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                title(s), style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis,
                fontWeight = if (run?.state?.live == true) FontWeight.SemiBold else FontWeight.Normal,
                color = if (selected) scheme.onSecondaryContainer else scheme.onSurface,
            )
            val sub = when {
                run != null && run.state.live -> runLine(run, now)
                s.latest != null -> listOfNotNull(s.latestSender?.let { shortName(it, it) }, s.latest.lineSequence().firstOrNull()).joinToString(": ")
                else -> null
            }
            sub?.let {
                Text(
                    it, style = MaterialTheme.typography.bodySmall, maxLines = 1, overflow = TextOverflow.Ellipsis,
                    color = if (run != null && run.state.live) run.state.color() else scheme.onSurfaceVariant,
                )
            }
        }
        Spacer(Modifier.width(8.dp))
        Column(horizontalAlignment = Alignment.End) {
            Text(elapsed(s.lastTs, now), style = MaterialTheme.typography.labelSmall, color = scheme.onSurfaceVariant)
            if (run != null) {
                Spacer(Modifier.size(4.dp))
                StateDot(run.state, 8.dp)
            }
        }
    }
}
