package org.fluxchat.companion.ui

import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.automirrored.outlined.OpenInNew
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.outlined.Mic
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.async
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.fluxchat.companion.Diag
import org.fluxchat.companion.agents.BusyMode
import org.fluxchat.companion.agents.Runs
import org.fluxchat.companion.matrix.Auth
import org.fluxchat.companion.matrix.Matrix
import org.fluxchat.companion.quick.Quick
import org.fluxchat.companion.quick.QuickSend

/**
 * A new session from anywhere (widget, Quick Settings, a launcher shortcut):
 * a sheet over whatever you were doing, with the project, the request (typed
 * or spoken) and Send. It closes once sent; the live runs notification takes
 * it from there.
 *
 * What you type is a draft of its project until it's sent: a tap beside the
 * sheet (or Back, or the phone ringing) closes it, and the next time the
 * sheet opens for that project the text is there. [onOpen] is given the
 * text too, for the project's own composer, and says whether it took it
 * (it stays this sheet's draft otherwise).
 */
@Composable
fun QuickPromptSheet(roomArg: String?, voice: Boolean, onOpen: (String, String) -> Boolean, onDone: () -> Unit) {
    val context = LocalContext.current
    val app = context.applicationContext
    val auth by Matrix.auth.collectAsState()
    val rooms by Matrix.agentRooms.collectAsState()
    val list by Matrix.agentList.collectAsState()
    val pins by Matrix.pins.collectAsState()
    val ordered = remember(rooms, pins) { rooms.values.sortedWith(agentRoomOrder(pins)) }
    var roomId by rememberSaveable { mutableStateOf(roomArg ?: Quick.room(context)) }
    val room = rooms[roomId] ?: ordered.firstOrNull()
    /** The project the text is a draft of (known before the projects have loaded, when the sheet was opened for one). */
    val draftOf = room?.roomId ?: roomId
    var text by rememberSaveable { mutableStateOf(roomId?.let { Quick.draft(context, it) }.orEmpty()) }
    var sending by remember { mutableStateOf(false) }
    var picking by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val focus = remember { FocusRequester() }
    val dictate = rememberDictation { said -> text = withDictation(text, said) }
    var listened by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        if (!voice) runCatching { focus.requestFocus() }.onFailure { Diag.note("quick prompt: the text box didn't take focus (${it.message})") }
    }
    LaunchedEffect(dictate) {
        if (voice && !listened && dictate != null) {
            listened = true
            dictate()
        }
    }
    // The sheet opened without a project and it's known now (the last one
    // used, or the first): its draft, unless something's been typed already.
    LaunchedEffect(draftOf) {
        if (draftOf != null && text.isEmpty()) text = Quick.draft(context, draftOf)
    }
    // Kept as you type (a moment behind), so the draft also survives the
    // sheet being killed; and at once when it closes.
    LaunchedEffect(text, draftOf) {
        delay(400)
        if (draftOf != null && !sending) Quick.saveDraft(app, draftOf, text)
    }
    val latest by rememberUpdatedState(Triple(draftOf, text, sending))
    DisposableEffect(Unit) {
        onDispose {
            val (id, draft, busy) = latest
            // Mid-send the message's fate decides the draft's (QuickSend), not the text left in the box.
            if (id != null && !busy) Quick.saveDraft(app, id, draft)
        }
    }
    val send: () -> Unit = send@{
        val r = room ?: return@send
        val t = text.trim()
        if (t.isEmpty() || sending) return@send
        sending = true
        Quick.saveDraft(app, r.roomId, text)
        val agent = r.agents.firstOrNull()
        val out = Runs.outgoing(list.harnessOf(agent ?: ""), t, false, BusyMode.Send, agent?.takeIf { !r.isDm })
        // The send itself on the app's scope: closing the sheet mustn't cancel it half-way.
        val sent = Matrix.scope.async { QuickSend.send(app, r.roomId, out) }
        scope.launch {
            val outcome = sent.await()
            sending = false
            when (outcome) {
                QuickSend.Outcome.Sent -> {
                    text = ""
                    Toast.makeText(app, "Sent to ${r.name}", Toast.LENGTH_SHORT).show()
                    onDone()
                }
                // In the queue, the server hasn't confirmed it: not "Sent".
                QuickSend.Outcome.Queued -> {
                    text = ""
                    Toast.makeText(app, "Sending to ${r.name}… It goes out as soon as there's a connection.", Toast.LENGTH_LONG).show()
                    onDone()
                }
                QuickSend.Outcome.Failed -> Toast.makeText(app, "Couldn't send to ${r.name}", Toast.LENGTH_LONG).show()
            }
        }
    }
    // While a send is under way the sheet stays: closing it then would say nothing about how it went.
    val dismiss: () -> Unit = { if (!sending) onDone() }
    BackHandler(enabled = sending) {}

    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.BottomCenter) {
        // The scrim, behind the sheet rather than around it: a tap on the
        // sheet itself never reaches it, and a screen reader gets one
        // "Close" target instead of the whole sheet read out as a button.
        Box(
            Modifier.matchParentSize().background(Color.Black.copy(alpha = 0.32f))
                .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null, role = Role.Button, onClick = dismiss)
                .semantics { contentDescription = "Close" },
        )
        Surface(
            shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp),
            color = MaterialTheme.colorScheme.surfaceContainer,
            modifier = Modifier.fillMaxWidth().imePadding(),
        ) {
            Column(Modifier.navigationBarsPadding().padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text("New session", style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                    room?.let { r ->
                        IconButton(enabled = !sending, onClick = {
                            // What's typed goes along, into the project's own message box.
                            if (onOpen(r.roomId, text)) text = ""
                        }) { Icon(Icons.AutoMirrored.Outlined.OpenInNew, "Open ${r.name}") }
                    }
                }
                when {
                    auth == Auth.SignedOut -> Text("Sign in to fluxchat first.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                    // Nothing will load by waiting: the app has a question to ask first.
                    auth == Auth.RestoreFailed || auth == Auth.NeedsRecovery ->
                        Text("Open fluxchat to finish signing in.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                    room == null -> Row(verticalAlignment = Alignment.CenterVertically) {
                        CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                        Spacer(Modifier.width(10.dp))
                        Text("Finding your agents…", color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    else -> Box {
                        Row(
                            Modifier.clip(RoundedCornerShape(50))
                                .clickable(onClickLabel = "Change project", role = Role.DropdownList) { picking = true }
                                .heightIn(min = 48.dp).padding(end = 8.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Avatar(room.roomId, room.name, room.avatar, 28.dp, room = !room.isDm)
                            Spacer(Modifier.width(10.dp))
                            Text(room.name, style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Icon(Icons.Filled.ArrowDropDown, null)
                        }
                        DropdownMenu(expanded = picking, onDismissRequest = { picking = false }, shape = MaterialTheme.shapes.medium) {
                            ordered.forEach { r ->
                                DropdownMenuItem(
                                    text = { Text(r.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                                    leadingIcon = { Avatar(r.roomId, r.name, r.avatar, 24.dp, room = !r.isDm) },
                                    onClick = {
                                        picking = false
                                        if (r.roomId != room.roomId) {
                                            // The text follows you to the other project (it's
                                            // no longer this one's draft); an empty box
                                            // shows what was left there.
                                            if (text.isNotBlank()) Quick.saveDraft(app, room.roomId, "") else text = Quick.draft(app, r.roomId)
                                            roomId = r.roomId
                                        }
                                    },
                                )
                            }
                        }
                    }
                }
                TextField(
                    value = text,
                    onValueChange = { text = it },
                    // Not disabled: that drops the focus, the keyboard, and the sheet with it.
                    readOnly = sending,
                    modifier = Modifier.fillMaxWidth().focusRequester(focus),
                    placeholder = { Text("Ask ${room?.agents?.firstOrNull()?.let { shortName(it, null) } ?: "your agent"}…") },
                    minLines = 2,
                    maxLines = 8,
                    shape = RoundedCornerShape(20.dp),
                    colors = TextFieldDefaults.colors(
                        focusedIndicatorColor = Color.Transparent,
                        unfocusedIndicatorColor = Color.Transparent,
                        focusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHighest,
                        unfocusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHighest,
                    ),
                )
                Row(verticalAlignment = Alignment.CenterVertically) {
                    dictate?.let { FilledTonalIconButton(onClick = it, enabled = !sending) { Icon(Icons.Outlined.Mic, "Speak") } }
                    Spacer(Modifier.weight(1f))
                    Button(onClick = send, enabled = text.isNotBlank() && room != null && !sending) {
                        if (sending) CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                        else Icon(Icons.AutoMirrored.Filled.Send, null, Modifier.size(18.dp))
                        Spacer(Modifier.width(8.dp))
                        Text(if (sending) "Sending…" else "Send")
                    }
                }
            }
        }
    }
}
