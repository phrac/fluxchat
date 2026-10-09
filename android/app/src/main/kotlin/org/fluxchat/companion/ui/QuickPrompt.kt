package org.fluxchat.companion.ui

import android.widget.Toast
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.fluxchat.companion.agents.BusyMode
import org.fluxchat.companion.agents.Runs
import org.fluxchat.companion.matrix.Auth
import org.fluxchat.companion.matrix.Matrix
import org.fluxchat.companion.quick.Quick

/**
 * A new session from anywhere (widget, Quick Settings, a launcher shortcut):
 * a sheet over whatever you were doing, with the room, the request (typed or
 * spoken) and Send. It closes once sent; the live runs notification takes it
 * from there.
 */
@Composable
fun QuickPromptSheet(roomArg: String?, voice: Boolean, onOpen: (String) -> Unit, onDone: () -> Unit) {
    val context = LocalContext.current
    val auth by Matrix.auth.collectAsState()
    val rooms by Matrix.agentRooms.collectAsState()
    val list by Matrix.agentList.collectAsState()
    val ordered = remember(rooms) { rooms.values.sortedWith(AGENT_ROOM_ORDER) }
    var roomId by rememberSaveable { mutableStateOf(roomArg ?: Quick.room(context)) }
    val room = rooms[roomId] ?: ordered.firstOrNull()
    var text by rememberSaveable { mutableStateOf("") }
    var sending by remember { mutableStateOf(false) }
    var picking by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val focus = remember { FocusRequester() }
    val dictate = rememberDictation { said -> text = withDictation(text, said) }
    var listened by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(Unit) { if (!voice) runCatching { focus.requestFocus() } }
    LaunchedEffect(dictate) {
        if (voice && !listened && dictate != null) {
            listened = true
            dictate()
        }
    }
    val send: () -> Unit = send@{
        val r = room ?: return@send
        val t = text.trim()
        if (t.isEmpty() || sending) return@send
        sending = true
        scope.launch {
            val agent = r.agents.firstOrNull()
            val out = Runs.outgoing(list.harnessOf(agent ?: ""), t, false, BusyMode.Send, agent?.takeIf { !r.isDm })
            // Through the room's timeline (Markdown, local echo) when it's open, else straight to the room.
            val timeline = Matrix.roomTimeline(r.roomId)
            val ok = if (timeline != null) runCatching { timeline.send(out) }.isSuccess else Matrix.sendText(r.roomId, null, out, inReplyTo = null)
            sending = false
            if (ok) {
                Quick.remember(context, r.roomId)
                Toast.makeText(context, "Sent to ${r.name}", Toast.LENGTH_SHORT).show()
                onDone()
            } else {
                Toast.makeText(context, "Couldn't send to ${r.name}", Toast.LENGTH_LONG).show()
            }
        }
    }

    Box(
        Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.32f))
            .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null, onClick = onDone),
        contentAlignment = Alignment.BottomCenter,
    ) {
        Surface(
            shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp),
            color = MaterialTheme.colorScheme.surfaceContainer,
            modifier = Modifier.fillMaxWidth().imePadding()
                .clickable(interactionSource = remember { MutableInteractionSource() }, indication = null) {},
        ) {
            Column(Modifier.navigationBarsPadding().padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text("New session", style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                    room?.let { r -> IconButton(onClick = { onOpen(r.roomId) }) { Icon(Icons.AutoMirrored.Outlined.OpenInNew, "Open ${r.name}") } }
                }
                when {
                    auth == Auth.SignedOut -> Text("Sign in to fluxchat first.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                    room == null -> Row(verticalAlignment = Alignment.CenterVertically) {
                        CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                        Spacer(Modifier.width(10.dp))
                        Text("Finding your agents…", color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    else -> Box {
                        Row(
                            Modifier.clip(RoundedCornerShape(50)).clickable { picking = true }.padding(end = 8.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Avatar(room.roomId, room.name, room.avatar, 28.dp, room = !room.isDm)
                            Spacer(Modifier.width(10.dp))
                            Text(room.name, style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Icon(Icons.Filled.ArrowDropDown, "Change room")
                        }
                        DropdownMenu(expanded = picking, onDismissRequest = { picking = false }, shape = MaterialTheme.shapes.medium) {
                            ordered.forEach { r ->
                                DropdownMenuItem(
                                    text = { Text(r.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                                    leadingIcon = { Avatar(r.roomId, r.name, r.avatar, 24.dp, room = !r.isDm) },
                                    onClick = { roomId = r.roomId; picking = false },
                                )
                            }
                        }
                    }
                }
                TextField(
                    value = text,
                    onValueChange = { text = it },
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
                    dictate?.let { FilledTonalIconButton(onClick = it) { Icon(Icons.Outlined.Mic, "Speak") } }
                    Spacer(Modifier.weight(1f))
                    Button(onClick = send, enabled = text.isNotBlank() && room != null && !sending) {
                        if (sending) CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                        else Icon(Icons.AutoMirrored.Filled.Send, null, Modifier.size(18.dp))
                        Spacer(Modifier.width(8.dp))
                        Text("Send")
                    }
                }
            }
        }
    }
}
