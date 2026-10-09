package org.fluxchat.companion.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.Chat
import androidx.compose.material.icons.outlined.ErrorOutline
import androidx.compose.material.icons.outlined.ExpandLess
import androidx.compose.material.icons.outlined.ExpandMore
import androidx.compose.material.icons.outlined.Schedule
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.agents.RunInfo
import org.fluxchat.companion.agents.RunState
import org.fluxchat.companion.agents.Runs
import org.fluxchat.companion.agents.SendState
import java.text.DateFormat
import java.util.Calendar
import java.util.Date

/** Agent answers longer than this fold (the desktop's `LONG_ANSWER_LINES`). */
private const val LONG_ANSWER_LINES = 24
private const val AVATAR = 32

/** A day separator: "Today", "Yesterday", or the date. */
@Composable
fun DaySeparator(ts: Long) {
    val label = remember(ts) { dayLabel(ts) }
    Box(Modifier.fillMaxWidth().padding(vertical = 12.dp), contentAlignment = Alignment.Center) {
        Text(
            label,
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.background(MaterialTheme.colorScheme.surfaceContainerHigh, RoundedCornerShape(50)).padding(horizontal = 12.dp, vertical = 4.dp),
        )
    }
}

@Composable
private fun <T> remember(key: Any?, calc: () -> T): T = androidx.compose.runtime.remember(key) { calc() }

fun sameDay(a: Long, b: Long): Boolean {
    val ca = Calendar.getInstance().apply { timeInMillis = a }
    val cb = Calendar.getInstance().apply { timeInMillis = b }
    return ca.get(Calendar.YEAR) == cb.get(Calendar.YEAR) && ca.get(Calendar.DAY_OF_YEAR) == cb.get(Calendar.DAY_OF_YEAR)
}

private fun dayLabel(ts: Long): String {
    val now = System.currentTimeMillis()
    return when {
        sameDay(ts, now) -> "Today"
        sameDay(ts, now - 86_400_000) -> "Yesterday"
        else -> DateFormat.getDateInstance(DateFormat.MEDIUM).format(Date(ts))
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun MessageRow(
    m: Msg,
    continued: Boolean,
    lastInGroup: Boolean,
    isAgent: Boolean,
    inThread: Boolean,
    prompt: List<Pair<String, String>>,
    question: List<Pair<String, String>>?,
    onReact: (String) -> Unit,
    onAnswer: (String) -> Unit,
    onOpenThread: (() -> Unit)?,
    threadRun: RunInfo?,
    now: Long,
) {
    if (!m.isMessage) {
        if (m.body.isNotEmpty()) {
            Text(
                m.body, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.fillMaxWidth().padding(vertical = 6.dp), textAlign = androidx.compose.ui.text.style.TextAlign.Center,
            )
        }
        return
    }
    val own = m.isOwn
    val scheme = MaterialTheme.colorScheme
    val steps = if (isAgent) Runs.progressSteps(m.body) else null
    BoxWithConstraints(Modifier.fillMaxWidth().padding(start = 12.dp, end = 12.dp, top = if (continued) 2.dp else 10.dp)) {
        // Agents' answers get most of the width; people's bubbles less.
        val maxBubble = maxWidth * (if (isAgent) 0.92f else 0.8f)
        Row(Modifier.fillMaxWidth(), horizontalArrangement = if (own) Arrangement.End else Arrangement.Start, verticalAlignment = Alignment.Bottom) {
            if (!own) {
                if (lastInGroup) Avatar(m.sender, m.senderName, m.senderAvatar, AVATAR.dp) else Spacer(Modifier.width(AVATAR.dp))
                Spacer(Modifier.width(8.dp))
            }
            Column(horizontalAlignment = if (own) Alignment.End else Alignment.Start, modifier = Modifier.widthIn(max = maxBubble)) {
                if (!continued && !own) {
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(start = 12.dp, bottom = 2.dp)) {
                        Text(shortName(m.sender, m.senderName), style = MaterialTheme.typography.labelLarge, color = nickColor(m.sender), fontWeight = FontWeight.SemiBold)
                        if (isAgent) {
                            Spacer(Modifier.width(6.dp))
                            Text(
                                "AGENT", style = MaterialTheme.typography.labelSmall, color = scheme.onTertiaryContainer,
                                modifier = Modifier.background(scheme.tertiaryContainer, RoundedCornerShape(4.dp)).padding(horizontal = 5.dp, vertical = 1.dp),
                            )
                        }
                    }
                }
                val shape = bubbleShape(own, continued, lastInGroup)
                if (steps != null) {
                    ProgressLog(steps)
                } else {
                    Surface(
                        color = if (own) scheme.primaryContainer else scheme.surfaceContainerHigh,
                        contentColor = if (own) scheme.onPrimaryContainer else scheme.onSurface,
                        shape = shape,
                    ) {
                        Column(Modifier.padding(if (m.media != null) 4.dp else 0.dp)) {
                            m.media?.let { MessageImage(it) }
                            Column(Modifier.padding(horizontal = 14.dp - (if (m.media != null) 4.dp else 0.dp), vertical = if (m.media != null) 6.dp else 10.dp)) {
                                if (m.body.isNotBlank()) Answer(m.body, foldable = isAgent, color = if (own) scheme.onPrimaryContainer else scheme.onSurface)
                                Meta(m, own)
                            }
                        }
                    }
                }
                if (prompt.isNotEmpty()) {
                    PromptCard("Waiting for your answer") { ChoiceButtons(prompt, onReact) }
                }
                if (question != null) {
                    PromptCard(if (question.isEmpty()) "Answer below" else "Pick one, or answer below") {
                        if (question.isNotEmpty()) ChoiceButtons(question, onAnswer)
                    }
                }
                val shown = m.reactions.filter { r -> prompt.none { it.first == r.emoji } }
                if (shown.isNotEmpty()) {
                    FlowRow(Modifier.padding(top = 4.dp), horizontalArrangement = Arrangement.spacedBy(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        for (r in shown) {
                            Text(
                                if (r.users.size > 1) "${r.emoji} ${r.users.size}" else r.emoji,
                                style = MaterialTheme.typography.labelMedium,
                                modifier = Modifier.background(scheme.surfaceContainerHighest, RoundedCornerShape(50)).clickable { onReact(r.emoji) }
                                    .padding(horizontal = 8.dp, vertical = 3.dp),
                            )
                        }
                    }
                }
                if (onOpenThread != null && !inThread) ThreadLink(m, threadRun, now, onOpenThread)
            }
        }
    }
}

/** Tighter corners where a sender's messages meet, like modern messengers. */
private fun bubbleShape(own: Boolean, continued: Boolean, last: Boolean): RoundedCornerShape {
    val big = 20.dp
    val small = 6.dp
    return if (own) {
        RoundedCornerShape(topStart = big, topEnd = if (continued) small else big, bottomEnd = if (last) big else small, bottomStart = big)
    } else {
        RoundedCornerShape(topStart = if (continued) small else big, topEnd = big, bottomEnd = big, bottomStart = if (last) big else small)
    }
}

@Composable
internal fun Meta(m: Msg, own: Boolean) {
    val c = MaterialTheme.colorScheme.onSurfaceVariant
    Row(Modifier.padding(top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        if (m.edited) Text("edited · ", style = MaterialTheme.typography.labelSmall, color = c)
        Text(DateFormat.getTimeInstance(DateFormat.SHORT).format(Date(m.ts)), style = MaterialTheme.typography.labelSmall, color = c)
        when (m.sendState) {
            SendState.Sending -> Icon(Icons.Outlined.Schedule, "Sending", Modifier.padding(start = 4.dp).size(12.dp), tint = c)
            SendState.Failed -> Icon(Icons.Outlined.ErrorOutline, "Failed to send", Modifier.padding(start = 4.dp).size(12.dp), tint = MaterialTheme.colorScheme.error)
            SendState.Sent -> {}
        }
    }
}

@Composable
private fun ThreadLink(m: Msg, run: RunInfo?, now: Long, open: () -> Unit) {
    Row(
        Modifier.padding(top = 6.dp).background(MaterialTheme.colorScheme.secondaryContainer, RoundedCornerShape(50)).clickable(onClick = open)
            .padding(horizontal = 12.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (run != null) StateDot(run.state, 8.dp) else Icon(Icons.AutoMirrored.Outlined.Chat, null, Modifier.size(14.dp), tint = MaterialTheme.colorScheme.onSecondaryContainer)
        Spacer(Modifier.width(6.dp))
        val count = if (m.threadCount > 0) "${m.threadCount} ${if (m.threadCount == 1) "reply" else "replies"}" else "Thread"
        val status = run?.let { runLine(it, now) } ?: m.threadLatest?.lineSequence()?.firstOrNull()
        Text(
            listOfNotNull(count, status).joinToString(" · "),
            style = MaterialTheme.typography.labelLarge,
            color = run?.state?.color() ?: MaterialTheme.colorScheme.onSecondaryContainer,
            maxLines = 1, overflow = TextOverflow.Ellipsis,
        )
    }
}

/** A step log: the latest step, all of them on tap. */
@Composable
private fun ProgressLog(steps: List<String>) {
    var open by rememberSaveable { mutableStateOf(false) }
    Surface(color = MaterialTheme.colorScheme.surfaceContainer, shape = RoundedCornerShape(16.dp), onClick = { open = !open }) {
        Column(Modifier.padding(horizontal = 12.dp, vertical = 8.dp).animateContentSize()) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    if (steps.size == 1) steps[0] else "${steps.size} steps · ${steps.last()}",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1, overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f, fill = false),
                )
                if (steps.size > 1) Icon(if (open) Icons.Outlined.ExpandLess else Icons.Outlined.ExpandMore, null, Modifier.size(18.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            AnimatedVisibility(open && steps.size > 1) {
                Column(Modifier.padding(top = 4.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    for (s in steps) Text(s, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
    }
}

@Composable
internal fun Answer(
    body: String,
    foldable: Boolean,
    color: androidx.compose.ui.graphics.Color,
    style: androidx.compose.ui.text.TextStyle = MaterialTheme.typography.bodyMedium,
) {
    val long = foldable && body.lines().size > LONG_ANSWER_LINES
    var open by rememberSaveable { mutableStateOf(false) }
    Column(Modifier.animateContentSize()) {
        MarkdownText(body, color = color, maxLines = if (long && !open) LONG_ANSWER_LINES else Int.MAX_VALUE, style = style)
        if (long) {
            Text(
                if (open) "Show less" else "Show all",
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(top = 6.dp).clickable { open = !open },
            )
        }
    }
}

@Composable
internal fun PromptCard(title: String, content: @Composable () -> Unit) {
    val s = LocalStatus.current
    Surface(color = s.needsYouContainer, shape = RoundedCornerShape(20.dp), modifier = Modifier.padding(top = 6.dp).fillMaxWidth()) {
        Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                StateDot(RunState.NeedsYou, 8.dp)
                Spacer(Modifier.width(8.dp))
                Text(title, style = MaterialTheme.typography.labelLarge, color = s.needsYou)
            }
            content()
        }
    }
}

/** Answer buttons: the first (approve / the first choice) filled, the rest outlined. */
@Composable
fun ChoiceButtons(choices: List<Pair<String, String>>, onPick: (String) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.fillMaxWidth()) {
        choices.forEachIndexed { i, (key, label) ->
            val text = if (label == key) key else "$key  $label"
            if (i == 0) {
                Button(onClick = { onPick(key) }, modifier = Modifier.fillMaxWidth()) { Text(text, maxLines = 2, overflow = TextOverflow.Ellipsis) }
            } else {
                OutlinedButton(onClick = { onPick(key) }, modifier = Modifier.fillMaxWidth()) { Text(text, maxLines = 2, overflow = TextOverflow.Ellipsis) }
            }
        }
    }
}
