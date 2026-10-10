package org.fluxchat.companion.ui

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.outlined.Description
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.flow.MutableStateFlow
import org.fluxchat.companion.Diag
import org.fluxchat.companion.matrix.Attachment
import org.fluxchat.companion.matrix.Matrix
import org.fluxchat.companion.matrix.Uploads
import java.io.File
import java.util.UUID

/** Something shared into the app from another one: text, a file, or both. */
data class Shared(val text: String?, val attachment: Attachment?)

/**
 * What's been shared and not sent yet. The share screen picks the room; that
 * room's composer then takes it ([take]).
 */
object ShareInbox {
    val pending = MutableStateFlow<Shared?>(null)
    /** The room the user picked for it. */
    val target = MutableStateFlow<String?>(null)

    /** The shared item, once, for the room it was sent to. */
    fun take(roomId: String): Shared? {
        if (target.value != roomId) return null
        val s = pending.value
        pending.value = null
        target.value = null
        return s
    }

    /**
     * An ACTION_SEND intent as a [Shared]. The file is copied into our cache at
     * once: the read grant that came with the intent doesn't outlive it.
     */
    fun fromIntent(context: Context, intent: Intent): Shared? {
        if (intent.action != Intent.ACTION_SEND) return null
        val text = listOfNotNull(
            intent.getStringExtra(Intent.EXTRA_SUBJECT),
            intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString(),
        ).filter { it.isNotBlank() }.distinct().joinToString("\n").ifBlank { null }
        @Suppress("DEPRECATION")
        val uri: Uri? = if (android.os.Build.VERSION.SDK_INT >= 33) intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
        else intent.getParcelableExtra(Intent.EXTRA_STREAM)
        val attachment = uri?.let { u ->
            runCatching {
                val a = Uploads.describe(context, u)
                val dir = File(context.cacheDir, "files/shared/${UUID.randomUUID()}").apply { mkdirs() }
                val copy = File(dir, Uploads.safeName(a.name))
                context.contentResolver.openInputStream(u)?.use { input -> copy.outputStream().use { input.copyTo(it) } } ?: error("unreadable")
                a.copy(uri = androidx.core.content.FileProvider.getUriForFile(context, context.packageName + ".files", copy), size = copy.length())
            }.onFailure { Diag.error("Reading a shared file", it) }.getOrNull()
        }
        if (text == null && attachment == null) return null
        return Shared(text, attachment)
    }
}

/** "Send to…": the agent rooms, for something shared from another app. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ShareScreen(back: () -> Unit, swap: (Screen) -> Unit) {
    val shared by ShareInbox.pending.collectAsState()
    val rooms by Matrix.agentRooms.collectAsState()
    val now by rememberNow(60_000)
    val sorted = rooms.values.sortedWith(AGENT_ROOM_ORDER)
    Scaffold(topBar = {
        TopAppBar(
            navigationIcon = { IconButton(onClick = { ShareInbox.pending.value = null; back() }) { Icon(Icons.Filled.Close, "Cancel") } },
            title = { Text("Send to…") },
        )
    }) { pad ->
        LazyColumn(Modifier.fillMaxSize().padding(pad), contentPadding = PaddingValues(bottom = 24.dp)) {
            shared?.let { s ->
                item {
                    Surface(
                        color = MaterialTheme.colorScheme.surfaceContainerHigh, shape = RoundedCornerShape(16.dp),
                        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
                    ) {
                        Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                            s.attachment?.let { a ->
                                Row(verticalAlignment = Alignment.CenterVertically) {
                                    Icon(Icons.Outlined.Description, null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                                    Spacer(Modifier.width(8.dp))
                                    Text(a.name, style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                }
                            }
                            s.text?.let { Text(it, style = MaterialTheme.typography.bodyMedium, maxLines = 4, overflow = TextOverflow.Ellipsis) }
                        }
                    }
                }
            }
            if (sorted.isEmpty()) {
                item {
                    Text(
                        "No projects yet.", color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(24.dp),
                    )
                }
            }
            items(sorted, key = { it.roomId }) { r ->
                Row(
                    Modifier.fillMaxWidth().clickable {
                        ShareInbox.target.value = r.roomId
                        swap(Screen.Room(r.roomId))
                    }.padding(horizontal = 16.dp, vertical = 10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Avatar(r.roomId, r.name, r.avatar, 44.dp, room = !r.isDm)
                    Spacer(Modifier.width(14.dp))
                    Column(Modifier.weight(1f)) {
                        Text(r.name, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        val live = r.headline?.second?.takeIf { it.state.live }
                        Text(
                            listOfNotNull(if (r.isDm) "Direct message" else "Project", live?.state?.label).joinToString(" · "),
                            style = MaterialTheme.typography.bodySmall, color = live?.state?.color() ?: MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    r.last?.let { Text(elapsed(it.ts, now), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
                }
            }
        }
    }
}
