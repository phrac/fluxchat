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
