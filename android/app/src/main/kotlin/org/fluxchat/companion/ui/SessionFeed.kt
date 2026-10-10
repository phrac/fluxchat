package org.fluxchat.companion.ui

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.combinedClickable
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
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.Chat
import androidx.compose.material.icons.outlined.PushPin
import androidx.compose.material.icons.outlined.Schedule
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.agents.RunInfo
import org.fluxchat.companion.agents.RunState
import org.fluxchat.companion.agents.Runs
import org.fluxchat.companion.agents.SendState
import org.fluxchat.companion.agents.ViewKey
import org.fluxchat.companion.matrix.Matrix
import org.fluxchat.companion.matrix.Session
import org.fluxchat.companion.matrix.SessionList
import org.fluxchat.companion.matrix.pinnedFirst

/** How long a request the agent hasn't picked up reads as "waiting" rather than "no reply". */
private const val PENDING_WINDOW_MS = 10 * 60 * 1000L

/** Space between session cards. */
private val CARD_GAP = 12.dp

/** Lines of the latest answer a card shows. */
private const val SUMMARY_LINES = 4

/**
 * One session on a room's feed: a request (a top-level message) and its
 * thread, summed up.
 */
data class SessionCardInfo(
    val root: String,
    val title: String,
    /** The latest answer (the agent's, else anyone's), as plain lines. */
    val summary: String?,
    val summaryBy: String?,
    val replies: Int,
    val lastTs: Long,
    val run: RunInfo?,
    val own: Boolean,
    val sendState: SendState = SendState.Sent,
)

/**
 * A room's sessions, newest activity first: the server's thread list (all of
 * them) merged with what the room's timeline has loaded — requests you just
 * sent that have no thread yet, live replies, and other top-level messages,
 * so nothing in the room goes unseen.
 */
fun sessionCards(roomId: String, sessions: List<Session>, all: List<Msg>, runs: Map<ViewKey, RunInfo?>, agents: List<String>): List<SessionCardInfo> {
    val replies = all.filter { it.threadRoot != null && it.isMessage }.groupBy { it.threadRoot!! }
    val top = all.filter { it.threadRoot == null && it.isMessage }.associateBy { it.id }
    val server = sessions.associateBy { it.root }

    fun card(root: String): SessionCardInfo {
        val msg = top[root]
        val s = server[root]
        val loaded = replies[root].orEmpty()
        val rootMsg = msg ?: s?.let { Msg(it.root, "", "", it.rootBody, it.rootTs, isOwn = false) }
        val answers = loaded.filter { it.body.isNotBlank() && Runs.progressSteps(it.body) == null }
        val answer = answers.lastOrNull { it.sender in agents } ?: answers.lastOrNull()
        val latest = s?.latest?.takeIf { s.lastTs > (answer?.ts ?: 0) && Runs.progressSteps(it) == null }
        val title = threadTitle(rootMsg, answers.ifEmpty { listOfNotNull(latest?.let { Msg("", "", "", it, s.lastTs, isOwn = false) }) })
        val summary = when {
            latest != null -> plainSummary(latest)
            answer != null -> plainSummary(answer.body)
            // A message nobody's answered: the rest of it under its first line.
            msg != null && !msg.isOwn -> plainSummary(msg.body.lineSequence().dropWhile { it.isBlank() }.drop(1).joinToString("\n"))
            else -> null
        }?.takeIf { it.isNotBlank() && it != title }
        return SessionCardInfo(
            root = root,
            title = title,
            summary = summary,
            summaryBy = if (latest != null) s.latestSender else answer?.senderName ?: msg?.takeIf { !it.isOwn }?.senderName,
            replies = maxOf(s?.replies ?: 0, msg?.threadCount ?: 0, loaded.size),
            lastTs = maxOf(s?.lastTs ?: 0, loaded.maxOfOrNull { it.ts } ?: 0, rootMsg?.ts ?: 0),
            run = runs[ViewKey(roomId, root)],
            own = msg?.isOwn ?: false,
            sendState = msg?.sendState ?: SendState.Sent,
        )
    }

    val roots = LinkedHashSet<String>().apply {
        addAll(server.keys)
        addAll(top.keys)
        // Threads whose root is out of the loaded history but whose replies came in live.
        addAll(replies.keys)
    }
    return roots.map(::card).sortedByDescending { it.lastTs }
}

/** An answer's opening lines without markdown marks, for a card. */
fun plainSummary(body: String): String {
    var inCode = false
    return body.lineSequence()
        .map { it.trim() }
        .filter { l ->
            if (l.startsWith("```")) { inCode = !inCode; false } else !inCode && l.isNotEmpty()
        }
        .map { l ->
            l.replace(Regex("^#{1,6}\\s+"), "")
                .replace(Regex("^>\\s?"), "")
                .replace(Regex("^[-*+]\\s+"), "• ")
                .replace("**", "").replace("__", "").replace("`", "")
        }
        .take(SUMMARY_LINES)
        .joinToString("\n")
}

/**
 * A room as its sessions: a card per request with where it stands and the
 * agent's latest answer. For rooms where each message starts its own thread.
 */
@Composable
fun SessionFeed(
    roomId: String,
    all: List<Msg>,
    agents: List<String>,
    runs: Map<ViewKey, RunInfo?>,
    list: SessionList?,
    now: Long,
    onOpen: (String) -> Unit,
) {
    val empty = remember { MutableStateFlow(emptyList<Session>()) }
    val sessions by (list?.items ?: empty).collectAsState()
    val loading by (list?.loading ?: MutableStateFlow(list == null)).collectAsState()
    val end by (list?.endReached ?: MutableStateFlow(false)).collectAsState()
    val cards = remember(sessions, all, runs, agents) { sessionCards(roomId, sessions, all, runs, agents) }
    val pins by Matrix.pins.collectAsState()
    val state = rememberLazyListState()
    val scope = rememberCoroutineScope()
    val togglePin: (String) -> Unit = { root ->
        val on = !pins.isPinned(roomId, root)
        scope.launch { Matrix.setPinned(roomId, root, on) }
    }
    val nearEnd by remember { derivedStateOf { (state.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0) >= state.layoutInfo.totalItemsCount - 4 } }
    LaunchedEffect(nearEnd, sessions.size) {
        if (nearEnd && !end && !loading) list?.more()
    }
    // Pinned sessions sit at the top of the feed; the rest keep their order.
    val ordered = remember(cards, pins) { pinnedFirst(cards) { pins.isPinned(roomId, it.root) } }
    val split = remember(ordered, pins) { ordered.indexOfFirst { !pins.isPinned(roomId, it.root) }.let { if (it < 0) ordered.size else it } }
    val pinned = ordered.subList(0, split)
    val rest = ordered.subList(split, ordered.size)
    // Newest at the bottom, by the composer, like a chat: what you just sent is where you look.
    FollowNewest(state, cards)
    LazyColumn(
        state = state,
        reverseLayout = true,
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(horizontal = 12.dp, vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(CARD_GAP),
    ) {
        var i = 0
        while (i < rest.size) {
            val b = bucket(rest[i].lastTs, now)
            var j = i
            while (j < rest.size && bucket(rest[j].lastTs, now) == b) j++
            for (c in rest.subList(i, j)) {
                item(key = c.root) { SessionCard(c, now, Modifier.animateItem(), pinned = false, onTogglePin = { togglePin(c.root) }) { onOpen(c.root) } }
            }
            // reverseLayout: the heading goes after its cards to sit above them.
            item(key = "h:$b") {
                Text(
                    b, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.padding(start = 8.dp, top = 12.dp, bottom = 2.dp),
                )
            }
            i = j
        }
        // Last emitted = top of a reverse-layout list: the pinned block sits above everything,
        // newest emitted first so (newest at the bottom, like the rest) the newest pin ends up
        // nearest the list below it, with the heading on top.
        for (c in pinned) {
            item(key = c.root) { SessionCard(c, now, Modifier.animateItem(), pinned = true, onTogglePin = { togglePin(c.root) }) { onOpen(c.root) } }
        }
        if (pinned.isNotEmpty()) {
            item(key = "h:pinned") {
                Text(
                    "Pinned", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.padding(start = 8.dp, top = 12.dp, bottom = 2.dp),
                )
            }
        }
        if (loading) {
            item(key = "loading") { Box(Modifier.fillMaxWidth().padding(16.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(Modifier.size(24.dp), strokeWidth = 2.dp) } }
        } else if (ordered.isEmpty()) {
            item(key = "empty") {
                Text(
                    "No sessions yet. Each message you send starts one.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 24.dp),
                )
            }
        }
    }
}

@Composable
fun SessionCard(c: SessionCardInfo, now: Long, modifier: Modifier = Modifier, pinned: Boolean = false, onTogglePin: () -> Unit = {}, onClick: () -> Unit) {
    val scheme = MaterialTheme.colorScheme
    val run = c.run
    val live = run != null && run.state.live
    val needsYou = run?.state == RunState.NeedsYou
    var menu by remember { mutableStateOf(false) }
    // Raised from the page and outlined, so neighbouring cards read as separate.
    // Long-press anywhere on the card: pin it so it stays on top.
    Card(
        modifier = modifier
            .fillMaxWidth()
            .animateContentSize()
            .combinedClickable(onClick = onClick, onLongClick = { menu = true }),
        shape = MaterialTheme.shapes.large,
        colors = CardDefaults.cardColors(
            containerColor = if (needsYou) LocalStatus.current.needsYouContainer else scheme.surfaceContainerHigh,
        ),
        border = androidx.compose.foundation.BorderStroke(
            1.dp, if (live) run.state.color().copy(alpha = 0.5f) else scheme.outlineVariant,
        ),
        elevation = CardDefaults.cardElevation(defaultElevation = 1.dp),
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(verticalAlignment = Alignment.Top) {
                if (pinned) {
                    Icon(Icons.Outlined.PushPin, "Pinned", Modifier.size(16.dp).align(Alignment.CenterVertically), tint = scheme.primary)
                    Spacer(Modifier.width(6.dp))
                }
                Text(
                    c.title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold,
                    maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f),
                )
                Spacer(Modifier.width(8.dp))
                Text(elapsed(c.lastTs, now), style = MaterialTheme.typography.labelMedium, color = scheme.onSurfaceVariant)
            }
            when {
                live -> StatePill(run.state, runLine(run, now))
                c.sendState == SendState.Sending -> Waiting("Sending…")
                c.sendState == SendState.Failed -> Text("Failed to send", style = MaterialTheme.typography.labelMedium, color = scheme.error)
                run == null && c.own && c.replies == 0 ->
                    Waiting(if (now - c.lastTs < PENDING_WINDOW_MS) "Waiting for the agent" else "No reply")
                else -> {}
            }
            c.summary?.let {
                Text(
                    if (c.summaryBy != null) "${shortName(c.summaryBy, c.summaryBy)}: $it" else it,
                    style = MaterialTheme.typography.bodyMedium, color = scheme.onSurfaceVariant,
                    maxLines = SUMMARY_LINES, overflow = TextOverflow.Ellipsis,
                )
            }
            if (!live && (run != null || c.replies > 0)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    if (run != null) {
                        StateDot(run.state, 8.dp)
                        Spacer(Modifier.width(6.dp))
                        Text(run.state.label, style = MaterialTheme.typography.labelMedium, color = run.state.color())
                        Spacer(Modifier.width(12.dp))
                    }
                    if (c.replies > 0) {
                        Icon(Icons.AutoMirrored.Outlined.Chat, null, Modifier.size(14.dp), tint = scheme.onSurfaceVariant)
                        Spacer(Modifier.width(4.dp))
                        Text(
                            "${c.replies} ${if (c.replies == 1) "reply" else "replies"}",
                            style = MaterialTheme.typography.labelMedium, color = scheme.onSurfaceVariant,
                        )
                    }
                    run?.costUsd?.let {
                        Spacer(Modifier.width(12.dp))
                        Text("$" + String.format(java.util.Locale.US, "%.2f", it), style = MaterialTheme.typography.labelMedium, color = scheme.onSurfaceVariant)
                    }
                }
            }
            when {
                run?.progress != null && live -> LinearProgressIndicator(progress = { run.progress }, modifier = Modifier.fillMaxWidth(), color = run.state.color())
                run?.state == RunState.Working -> LinearProgressIndicator(Modifier.fillMaxWidth(), color = run.state.color())
                else -> {}
            }
        }
        DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
            DropdownMenuItem(
                text = { Text(if (pinned) "Unpin session" else "Pin session") },
                leadingIcon = { Icon(Icons.Outlined.PushPin, null) },
                onClick = { menu = false; onTogglePin() },
            )
        }
    }
}

@Composable
private fun Waiting(text: String) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Icon(Icons.Outlined.Schedule, null, Modifier.size(14.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.width(6.dp))
        Text(text, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}
