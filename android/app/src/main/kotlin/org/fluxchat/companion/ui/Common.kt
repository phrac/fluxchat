package org.fluxchat.companion.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.agents.RunInfo
import org.fluxchat.companion.agents.RunState
import org.fluxchat.companion.agents.Runs

/**
 * Whether the app's content may show. Dialogs, sheets and menus draw in
 * windows of their own, above the lock screen (an overlay inside the
 * activity's window): a full-screen photo stayed up over "fluxchat is
 * locked", and a "Leave this project?" could be answered from there. Each of
 * them checks this.
 */
@Composable
fun unlocked(): Boolean = !org.fluxchat.companion.AppLock.locked.collectAsState().value

/** Pin or unpin a session, and say so when it didn't take: a failed pin used to do nothing you could see. */
suspend fun setPinnedOrSay(context: android.content.Context, roomId: String, root: String, pinned: Boolean) {
    if (org.fluxchat.companion.matrix.Matrix.setPinned(roomId, root, pinned)) return
    // Said here, where it happened — not later in red in some other dialog.
    org.fluxchat.companion.matrix.Matrix.clearError()
    kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.Main) {
        android.widget.Toast.makeText(
            context,
            if (pinned) "Couldn't pin the session. Check your connection." else "Couldn't unpin the session. Check your connection.",
            android.widget.Toast.LENGTH_LONG,
        ).show()
    }
}

@Composable
fun StateDot(state: RunState, size: Dp = 10.dp) {
    Box(Modifier.size(size).background(state.color(), CircleShape))
}

/** A small rounded label in the state's colour: "Working · 3m". */
@Composable
fun StatePill(state: RunState, text: String, modifier: Modifier = Modifier) {
    val c = state.color()
    androidx.compose.foundation.layout.Row(
        modifier.background(c.copy(alpha = 0.14f), androidx.compose.foundation.shape.RoundedCornerShape(50))
            .padding(horizontal = 10.dp, vertical = 4.dp),
        verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
    ) {
        if (state == RunState.Working) {
            androidx.compose.material3.CircularProgressIndicator(
                modifier = Modifier.size(12.dp), color = c, strokeWidth = 2.dp,
            )
        } else {
            StateDot(state, 8.dp)
        }
        androidx.compose.foundation.layout.Spacer(Modifier.size(6.dp))
        androidx.compose.material3.Text(
            text, color = c, style = androidx.compose.material3.MaterialTheme.typography.labelMedium,
            maxLines = 1, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
        )
    }
}

/** The clock, ticking every [everyMs] (for "3m" elapsed labels). */
@Composable
fun rememberNow(everyMs: Long = 15_000): State<Long> {
    val now = remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(everyMs) {
        while (true) {
            delay(everyMs)
            now.longValue = System.currentTimeMillis()
        }
    }
    return now
}

fun elapsed(since: Long, now: Long): String {
    val s = ((now - since) / 1000).coerceAtLeast(0)
    return when {
        s < 60 -> "${s}s"
        s < 3600 -> "${s / 60}m"
        s < 86400 -> "${s / 3600}h ${(s % 3600) / 60}m"
        else -> "${s / 86400}d"
    }
}

/** "Working · 3m · 🔧 Running tests (4)". */
fun runLine(run: RunInfo, now: Long): String = buildList {
    add(run.state.label)
    if (run.state.live) add(elapsed(run.since, now))
    // A finished run's last step is old news; its result isn't.
    if (run.state.live) Runs.stepLabel(run.step)?.let { add(if (run.steps > 1) "$it (${run.steps})" else it) }
    run.costUsd?.let { add("$" + String.format(java.util.Locale.US, "%.2f", it)) }
}.joinToString(" · ")

/** A finished run's one-line outcome (protocol `result`), trimmed; null when
 * the agent didn't report one. */
fun runResult(run: RunInfo): String? =
    if (run.state.live) null else run.result?.trim()?.takeIf { it.isNotEmpty() }?.lineSequence()?.firstOrNull()?.take(160)

/** A thread's title: its root's first line (unless that says nothing), else the latest reply. */
fun threadTitle(root: Msg?, replies: List<Msg>): String {
    val first = root?.body?.lineSequence()?.map { it.trim() }?.firstOrNull { it.isNotEmpty() }
    val meaningful = first != null && first.length > 1 && !first.startsWith("!") && !first.startsWith("/")
    if (meaningful) return first.take(120)
    return replies.lastOrNull { it.isMessage && Runs.progressSteps(it.body) == null }?.body?.lineSequence()?.firstOrNull()?.take(120)
        ?: first ?: org.fluxchat.companion.agents.History.UNTITLED
}

/** A sender's short name: display name, else the localpart. */
fun shortName(userId: String, name: String?): String =
    name?.takeIf { it.isNotBlank() && it != userId } ?: userId.removePrefix("@").substringBefore(':')

/**
 * Keep a bottom-anchored (`reverseLayout`) list at the newest item while you
 * are there. Compose holds the list on the item you were looking at, so new
 * items otherwise land below the view. Scrolled up to read, it stays put.
 */
@Composable
fun FollowNewest(state: androidx.compose.foundation.lazy.LazyListState, content: Any?, enabled: Boolean = true) {
    var follow by androidx.compose.runtime.remember { androidx.compose.runtime.mutableStateOf(true) }
    // Decided only when a scroll ends: a new item arriving shifts the index
    // too, and that mustn't count as you scrolling away.
    LaunchedEffect(state) {
        androidx.compose.runtime.snapshotFlow { state.isScrollInProgress }.collect { scrolling ->
            if (!scrolling) follow = state.firstVisibleItemIndex == 0 && state.firstVisibleItemScrollOffset < FOLLOW_SLACK_PX
        }
    }
    LaunchedEffect(content, enabled) {
        if (!enabled) return@LaunchedEffect
        // Re-check on restart: re-enabled after a deep-link jump, the view is
        // wherever the jump left it, and only following at the bottom is right.
        follow = state.firstVisibleItemIndex == 0 && state.firstVisibleItemScrollOffset < FOLLOW_SLACK_PX
        if (follow) state.scrollToItem(0)
    }
}

/** How far from the bottom still counts as "at the bottom", in pixels. */
private const val FOLLOW_SLACK_PX = 64
