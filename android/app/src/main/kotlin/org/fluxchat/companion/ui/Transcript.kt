package org.fluxchat.companion.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.BorderStroke
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
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Build
import androidx.compose.material.icons.outlined.CheckCircle
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material.icons.outlined.ErrorOutline
import androidx.compose.material3.IconButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.ui.draw.clip
import androidx.compose.material.icons.outlined.ExpandLess
import androidx.compose.material.icons.outlined.ExpandMore
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.agents.Reaction
import org.fluxchat.companion.agents.Runs

/** Side margin of the transcript: answers run the full width inside it. */
private val GUTTER = 16.dp

/** How wide your prompts get, as a share of the screen. */
private const val PROMPT_WIDTH = 0.85f

/**
 * A session as a transcript, the way Hermes's desktop app shows it rather
 * than a chat: your prompts as blocks on the right, the agent's answers as
 * full-width text with no bubble, its tool work as a collapsible activity
 * block. Approvals and questions are the same cards as in the chat.
 */
@Composable
fun TranscriptRow(
    m: Msg,
    prev: Msg?,
    isAgent: Boolean,
    /** The agents' own status reactions (👀 ✅ ❌): the run card says it, so they're left out. */
    hiddenReactions: (Reaction) -> Boolean,
    prompt: List<Pair<String, String>>,
    question: List<Pair<String, String>>?,
    /** This step log is the run's current activity (spinner, open by default). */
    live: Boolean,
    onReact: (String) -> Unit,
    onAnswer: (String) -> Unit,
    /** An approval you answered ("✅ Approve once"): folded to one line. */
    answered: String? = null,
) {
    if (!m.isMessage) {
        if (m.body.isNotEmpty()) {
            Text(
                m.body, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.fillMaxWidth().padding(vertical = 6.dp), textAlign = TextAlign.Center,
            )
        }
        return
    }
    val reactions = m.reactions.filter { r -> !hiddenReactions(r) && prompt.none { it.first == r.emoji } }
    if (isAgent) {
        val turnStart = prev == null || prev.sender != m.sender || !prev.isMessage
        Column(Modifier.fillMaxWidth().padding(start = GUTTER, end = GUTTER, top = if (turnStart) 18.dp else 8.dp)) {
            if (turnStart) {
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(bottom = 8.dp)) {
                    Avatar(m.sender, m.senderName, m.senderAvatar, 22.dp)
                    Spacer(Modifier.width(8.dp))
                    Text(shortName(m.sender, m.senderName), style = MaterialTheme.typography.labelLarge, fontWeight = FontWeight.SemiBold)
                    Spacer(Modifier.width(8.dp))
                    Text(time(m.ts), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            val steps = Runs.progressSteps(m.body)
            if (answered != null) {
                Answered(m.body, answered)
            } else if (Runs.isHeartbeat(m.body)) {
                Text(
                    clean(m.body.trim()), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(start = 4.dp, top = 2.dp, bottom = 2.dp),
                )
            } else if (steps != null) {
                Activity(steps, live)
            } else {
                m.media?.let { Box(Modifier.padding(bottom = 6.dp)) { MessageImage(it) } }
                m.file?.let { FileCard(it, Modifier.padding(bottom = 6.dp)) }
                if (m.body.isNotBlank()) {
                    Answer(m.body, foldable = false, color = MaterialTheme.colorScheme.onSurface, style = MaterialTheme.typography.bodyLarge)
                }
                if (m.edited) Text("edited", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            if (prompt.isNotEmpty()) PromptCard("Waiting for your answer") { ChoiceButtons(prompt, onReact) }
            if (question != null) {
                PromptCard(if (question.isEmpty()) "Answer below" else "Pick one, or answer below") {
                    if (question.isNotEmpty()) ChoiceButtons(question, onAnswer)
                }
            }
            if (answered == null) Reactions(reactions, onReact)
        }
    } else {
        val own = m.isOwn
        BoxWithConstraints(Modifier.fillMaxWidth().padding(start = GUTTER, end = GUTTER, top = 22.dp)) {
            val promptMax = maxWidth * PROMPT_WIDTH
            Column(Modifier.fillMaxWidth(), horizontalAlignment = if (own) Alignment.End else Alignment.Start) {
                if (!own) {
                    Text(
                        shortName(m.sender, m.senderName), style = MaterialTheme.typography.labelLarge, color = nickColor(m.sender),
                        fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(start = 4.dp, bottom = 4.dp),
                    )
                }
                Surface(
                    color = MaterialTheme.colorScheme.surfaceContainerHighest,
                    shape = RoundedCornerShape(18.dp),
                    modifier = Modifier.widthIn(max = promptMax),
                ) {
                    Column(Modifier.padding(if (m.media != null) 4.dp else 0.dp)) {
                        m.media?.let { MessageImage(it) }
                        m.file?.let { FileCard(it, Modifier.padding(4.dp).widthIn(min = 240.dp)) }
                        if (m.body.isNotBlank()) {
                            Box(Modifier.padding(horizontal = 14.dp, vertical = 10.dp)) {
                                MarkdownText(m.body, style = MaterialTheme.typography.bodyLarge)
                            }
                        }
                    }
                }
                Box(Modifier.padding(horizontal = 4.dp)) { Meta(m, own) }
                Reactions(reactions, onReact)
            }
        }
    }
}

/** Tool work: "6 tool calls" and the latest step; every step on tap. */
@Composable
private fun Activity(steps: List<String>, live: Boolean) {
    // The current run's activity starts open; finished ones fold away.
    var open by rememberSaveable(live) { mutableStateOf(live) }
    val scheme = MaterialTheme.colorScheme
    Surface(
        color = scheme.surfaceContainerLow,
        shape = RoundedCornerShape(14.dp),
        border = BorderStroke(1.dp, scheme.outlineVariant),
        onClick = { open = !open },
        modifier = Modifier.fillMaxWidth(),
    ) {
        Column(Modifier.animateContentSize()) {
            Row(Modifier.padding(horizontal = 12.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                if (live) {
                    CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp, color = org.fluxchat.companion.agents.RunState.Working.color())
                } else {
                    Icon(Icons.Outlined.Build, null, Modifier.size(16.dp), tint = scheme.onSurfaceVariant)
                }
                Spacer(Modifier.width(10.dp))
                Column(Modifier.weight(1f)) {
                    Text(
                        if (steps.size == 1) "1 tool call" else "${steps.size} tool calls",
                        style = MaterialTheme.typography.labelLarge, color = scheme.onSurface,
                    )
                    if (!open) {
                        Text(clean(steps.last()), style = MaterialTheme.typography.bodySmall, color = scheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
                Icon(if (open) Icons.Outlined.ExpandLess else Icons.Outlined.ExpandMore, if (open) "Hide steps" else "Show steps", Modifier.size(20.dp), tint = scheme.onSurfaceVariant)
            }
            AnimatedVisibility(open) {
                Column(Modifier.padding(start = 12.dp, end = 12.dp, bottom = 10.dp)) {
                    steps.forEachIndexed { i, s -> Step(s, last = i == steps.lastIndex, live = live && i == steps.lastIndex) }
                }
            }
        }
    }
}

/** An approval that's been answered: what it asked and your answer; the whole request on tap. */
@Composable
private fun Answered(body: String, answer: String) {
    var open by rememberSaveable { mutableStateOf(false) }
    val scheme = MaterialTheme.colorScheme
    val asked = body.lineSequence().map { it.trim().trim('*') }.firstOrNull { it.isNotEmpty() && !it.startsWith("```") }.orEmpty().replace("`", "").replace("**", "").replace("__", "")
    Surface(
        color = scheme.surfaceContainerLow, shape = RoundedCornerShape(14.dp), border = BorderStroke(1.dp, scheme.outlineVariant),
        onClick = { open = !open }, modifier = Modifier.fillMaxWidth(),
    ) {
        Column(Modifier.animateContentSize()) {
            Row(Modifier.padding(horizontal = 12.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(answer, style = MaterialTheme.typography.labelLarge, color = scheme.onSurface)
                    Text(asked, style = MaterialTheme.typography.bodySmall, color = scheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                Icon(if (open) Icons.Outlined.ExpandLess else Icons.Outlined.ExpandMore, if (open) "Hide request" else "Show request", Modifier.size(20.dp), tint = scheme.onSurfaceVariant)
            }
            AnimatedVisibility(open) {
                Box(Modifier.padding(start = 12.dp, end = 12.dp, bottom = 12.dp)) { MarkdownText(body, style = MaterialTheme.typography.bodyMedium) }
            }
        }
    }
}

/** One step: its emoji on a rail, then what it did. */
@Composable
private fun Step(line: String, last: Boolean, live: Boolean) {
    val scheme = MaterialTheme.colorScheme
    val emoji = line.substringBefore(' ')
    val text = clean(line.substringAfter(' '))
    Row(verticalAlignment = Alignment.Top) {
        Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.width(24.dp)) {
            Text(emoji, style = MaterialTheme.typography.bodySmall)
            if (!last) Box(Modifier.width(1.dp).height(10.dp).background(scheme.outlineVariant))
        }
        Spacer(Modifier.width(8.dp))
        Text(
            text, style = MaterialTheme.typography.bodySmall,
            color = if (live) scheme.onSurface else scheme.onSurfaceVariant,
            fontWeight = if (live) FontWeight.Medium else FontWeight.Normal,
            maxLines = 2, overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f).padding(top = 1.dp),
        )
    }
}

/** A step without its markdown backticks. */
private fun clean(step: String) = step.replace("`", "")

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Reactions(reactions: List<Reaction>, onReact: (String) -> Unit) {
    if (reactions.isEmpty()) return
    FlowRow(Modifier.padding(top = 6.dp), horizontalArrangement = Arrangement.spacedBy(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        for (r in reactions) {
            Text(
                if (r.users.size > 1) "${r.emoji} ${r.users.size}" else r.emoji,
                style = MaterialTheme.typography.labelMedium,
                modifier = Modifier.background(MaterialTheme.colorScheme.surfaceContainerHighest, RoundedCornerShape(50)).clickable { onReact(r.emoji) }
                    .padding(horizontal = 8.dp, vertical = 3.dp),
            )
        }
    }
}

private fun time(ts: Long): String = java.text.DateFormat.getTimeInstance(java.text.DateFormat.SHORT).format(java.util.Date(ts))

/** What a transcript shows, in order: messages, finished work folded away, and a turn's done line. */
sealed interface TranscriptEntry {
    val key: String

    /** A message as it is ([prev]: the message shown before it, for its header). */
    data class Row(val index: Int, val id: String, val prev: Msg?) : TranscriptEntry {
        override val key get() = id
    }

    /** A finished turn's work before its answer: tool logs, notes, heartbeats, answered approvals. */
    data class Work(val indices: List<Int>, val agent: Msg, val toolCalls: Int, val updates: Int, val durationMs: Long) : TranscriptEntry {
        override val key get() = "work:" + agent.id
    }

    /** A finished turn: its work folded, the answer, and how it went — one card. */
    data class Answer(val work: Work?, val answerIndex: Int, val id: String, val done: Done, val askedIndex: Int? = null) : TranscriptEntry {
        override val key get() = "answer:$id"
    }

    /** Under a finished turn's answer: how long it took and what it did. */
    data class Done(val answerIndex: Int, val id: String, val durationMs: Long, val toolCalls: Int, val latest: Boolean) : TranscriptEntry {
        override val key get() = "done:$id"
    }
}

/**
 * A session's messages as turns: your prompt, then the agent's work and its
 * answer. A finished turn folds its work into one [TranscriptEntry.Work] so
 * the answer stands out, with a [TranscriptEntry.Done] line under it. The
 * turn in progress ([live]) and any turn with a prompt or question still open
 * on you ([open]) stay as they are.
 */
fun transcriptEntries(msgs: List<Msg>, agents: Collection<String>, live: Boolean, open: (Int) -> Boolean): List<TranscriptEntry> {
    val out = ArrayList<TranscriptEntry>()
    fun row(i: Int) = TranscriptEntry.Row(i, msgs[i].id, msgs.getOrNull(i - 1))
    var i = 0
    while (i < msgs.size) {
        val m = msgs[i]
        if (m.isMessage && m.sender !in agents) {
            out += row(i)
            i++
            continue
        }
        // The agent's side of a turn: everything up to the next person's message.
        val start = i
        while (i < msgs.size && !(msgs[i].isMessage && msgs[i].sender !in agents)) i++
        val turn = (start until i).toList()
        val asked = msgs.getOrNull(start - 1)?.ts ?: msgs[start].ts
        val last = i == msgs.size
        val answer = turn.lastOrNull { j ->
            val a = msgs[j]
            a.isMessage && a.sender in agents && Runs.progressSteps(a.body) == null && !Runs.isHeartbeat(a.body) &&
                (a.body.isNotBlank() || a.file != null || a.media != null)
        }
        if ((last && live) || answer == null || turn.any(open)) {
            turn.forEach { out += row(it) }
            continue
        }
        val work = turn.filter { it != answer }
        val toolCalls = work.sumOf { Runs.progressSteps(msgs[it].body)?.size ?: 0 }
        val folded = work.takeIf { it.isNotEmpty() }?.let {
            val updates = work.count { msgs[it].isMessage && Runs.progressSteps(msgs[it].body) == null && !Runs.isHeartbeat(msgs[it].body) }
            TranscriptEntry.Work(work, msgs[work.first()], toolCalls, updates, msgs[answer].ts - asked)
        }
        out += TranscriptEntry.Answer(
            folded, answer, msgs[answer].id, TranscriptEntry.Done(answer, msgs[answer].id, msgs[answer].ts - asked, toolCalls, latest = last),
            askedIndex = (start - 1).takeIf { it >= 0 },
        )
    }
    return out
}

/** "6m", "45s", "1h 20m". */
private fun took(ms: Long): String {
    val s = (ms / 1000).coerceAtLeast(1)
    return when {
        s < 60 -> "${s}s"
        s < 3600 -> "${s / 60}m"
        else -> "${s / 3600}h ${(s % 3600) / 60}m"
    }
}

/**
 * A finished turn as one card: the agent, "Worked for 6m · 31 tool calls ·
 * 2 updates" (tap for all of it: [work]), the answer, and how it went. Raised
 * and outlined, so each answer reads as a unit, apart from your prompts.
 */
@Composable
fun TranscriptAnswer(
    e: TranscriptEntry.Answer,
    answer: Msg,
    run: org.fluxchat.companion.agents.RunInfo?,
    /** How the turn ended: the agent's ✅ / ❌ on the request, else the run's state. */
    outcome: org.fluxchat.companion.agents.RunState = org.fluxchat.companion.agents.RunState.Done,
    work: @Composable () -> Unit,
) {
    var open by rememberSaveable(e.key) { mutableStateOf(false) }
    val scheme = MaterialTheme.colorScheme
    // A faint wash of the outcome's colour (green done, red failed), stronger on the border.
    val hue = outcome.color()
    Surface(
        color = androidx.compose.ui.graphics.lerp(scheme.surfaceContainerLow, hue, OUTCOME_FILL),
        shape = RoundedCornerShape(20.dp),
        border = BorderStroke(1.dp, hue.copy(alpha = OUTCOME_BORDER)),
        modifier = Modifier.fillMaxWidth().padding(start = CARD_MARGIN, end = CARD_MARGIN, top = 18.dp),
    ) {
        Column(Modifier.padding(vertical = 12.dp).animateContentSize()) {
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(horizontal = CARD_PAD)) {
                Avatar(answer.sender, answer.senderName, answer.senderAvatar, 22.dp)
                Spacer(Modifier.width(8.dp))
                Text(shortName(answer.sender, answer.senderName), style = MaterialTheme.typography.labelLarge, fontWeight = FontWeight.SemiBold)
                Spacer(Modifier.width(8.dp))
                Text(time(answer.ts), style = MaterialTheme.typography.labelSmall, color = scheme.onSurfaceVariant)
            }
            e.work?.let { w ->
                Row(
                    Modifier.padding(start = CARD_PAD - 4.dp, top = 6.dp).clip(RoundedCornerShape(50)).clickable { open = !open }.padding(vertical = 4.dp, horizontal = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(Icons.Outlined.Build, null, Modifier.size(14.dp), tint = scheme.onSurfaceVariant)
                    Spacer(Modifier.width(8.dp))
                    Text(
                        listOfNotNull(
                            "Worked for ${took(w.durationMs)}",
                            w.toolCalls.takeIf { it > 0 }?.let { "$it tool call${if (it == 1) "" else "s"}" },
                            w.updates.takeIf { it > 0 }?.let { "$it update${if (it == 1) "" else "s"}" },
                        ).joinToString(" · "),
                        style = MaterialTheme.typography.labelLarge, color = scheme.onSurfaceVariant,
                    )
                    Icon(if (open) Icons.Outlined.ExpandLess else Icons.Outlined.ExpandMore, if (open) "Hide the work" else "Show the work", Modifier.size(18.dp), tint = scheme.onSurfaceVariant)
                }
                AnimatedVisibility(open) { Column(Modifier.padding(bottom = 8.dp)) { work() } }
            }
            Column(Modifier.padding(start = CARD_PAD, end = CARD_PAD, top = 10.dp)) {
                answer.media?.let { Box(Modifier.padding(bottom = 6.dp)) { MessageImage(it) } }
                answer.file?.let { FileCard(it, Modifier.padding(bottom = 6.dp)) }
                if (answer.body.isNotBlank()) {
                    Answer(answer.body, foldable = false, color = scheme.onSurface, style = MaterialTheme.typography.bodyLarge)
                }
            }
            HorizontalDivider(Modifier.padding(top = 12.dp, bottom = 2.dp), color = scheme.outlineVariant.copy(alpha = 0.6f))
            TranscriptDone(e.done, answer, run, outcome)
        }
    }
}

/** The card's distance from the screen edge, and its content's from the card's. */
private val CARD_MARGIN = 10.dp
private val CARD_PAD = 14.dp

/** How much of the outcome's colour tints a finished turn's card, and its border. */
private const val OUTCOME_FILL = 0.12f
private const val OUTCOME_BORDER = 0.35f

/** Under an answer: "✓ Done · 6m · 31 tool calls · $0.12", and copy the answer. */
@Composable
fun TranscriptDone(
    d: TranscriptEntry.Done,
    answer: Msg,
    run: org.fluxchat.companion.agents.RunInfo?,
    outcome: org.fluxchat.companion.agents.RunState = org.fluxchat.companion.agents.RunState.Done,
) {
    val scheme = MaterialTheme.colorScheme
    val clipboard = androidx.compose.ui.platform.LocalClipboardManager.current
    val failed = outcome == org.fluxchat.companion.agents.RunState.Failed
    Row(Modifier.fillMaxWidth().padding(start = CARD_PAD, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        Icon(
            if (failed) Icons.Outlined.ErrorOutline else Icons.Outlined.CheckCircle, null, Modifier.size(14.dp),
            tint = if (failed) org.fluxchat.companion.agents.RunState.Failed.color() else org.fluxchat.companion.agents.RunState.Done.color(),
        )
        Spacer(Modifier.width(6.dp))
        Text(
            listOfNotNull(
                if (failed) "Failed" else "Done",
                took(d.durationMs),
                d.toolCalls.takeIf { it > 0 }?.let { "$it tool call${if (it == 1) "" else "s"}" },
                run?.costUsd?.takeIf { d.latest }?.let { "$" + String.format(java.util.Locale.US, "%.2f", it) },
                run?.model?.takeIf { d.latest },
            ).joinToString(" · "),
            style = MaterialTheme.typography.labelMedium, color = scheme.onSurfaceVariant, modifier = Modifier.weight(1f),
            maxLines = 1, overflow = TextOverflow.Ellipsis,
        )
        IconButton(onClick = { clipboard.setText(androidx.compose.ui.text.AnnotatedString(answer.body)) }, modifier = Modifier.size(36.dp)) {
            Icon(Icons.Outlined.ContentCopy, "Copy the answer", Modifier.size(16.dp), tint = scheme.onSurfaceVariant)
        }
    }
}
