package org.fluxchat.companion.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
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
    // A finished run's last step is old news.
    if (run.state.live) run.step?.let { add(if (run.steps > 1) "$it (${run.steps})" else it) }
    run.costUsd?.let { add("$" + String.format(java.util.Locale.US, "%.2f", it)) }
}.joinToString(" · ")

/** A thread's title: its root's first line (unless that says nothing), else the latest reply. */
fun threadTitle(root: Msg?, replies: List<Msg>): String {
    val first = root?.body?.lineSequence()?.map { it.trim() }?.firstOrNull { it.isNotEmpty() }
    val meaningful = first != null && first.length > 1 && !first.startsWith("!") && !first.startsWith("/")
    if (meaningful) return first.take(120)
    return replies.lastOrNull { it.isMessage && Runs.progressSteps(it.body) == null }?.body?.lineSequence()?.firstOrNull()?.take(120)
        ?: first ?: "Thread"
}

/** A sender's short name: display name, else the localpart. */
fun shortName(userId: String, name: String?): String =
    name?.takeIf { it.isNotBlank() && it != userId } ?: userId.removePrefix("@").substringBefore(':')
