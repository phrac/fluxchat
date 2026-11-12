package org.fluxchat.companion.ui

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.outlined.Description
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.withContext
import org.fluxchat.companion.Diag
import org.fluxchat.companion.matrix.Attachment
import org.fluxchat.companion.matrix.Matrix
import org.fluxchat.companion.matrix.Uploads
import java.io.File
import java.util.UUID

/** Something shared into the app from another one: text, a file, or both. */
data class Shared(
    val text: String?,
    val attachment: Attachment?,
    /** What didn't come along, in words for the user (a file that couldn't be read). */
    val problem: String? = null,
    /** Our copy of the file, deleted when the share is over. */
    internal val file: File? = null,
)

/** A shared file on its way into our cache: its name, how much has been copied, and its size if the sender said. */
data class ShareCopy(val name: String, val copied: Long, val total: Long?)

/**
 * What's been shared and not sent yet. The share screen picks the room; that
 * room's composer then takes it ([take]).
 */
object ShareInbox {
    val pending = MutableStateFlow<Shared?>(null)
    /** The room the user picked for it. */
    val target = MutableStateFlow<String?>(null)

    private const val SHARED_DIR = "files/shared"

    /**
     * How long a copy that reached a composer is kept. Nothing tells the
     * inbox when the composer has sent it (or dropped it), and deleting it
     * under an unsent attachment would fail the send — so it outlives any
     * plausible wait, and goes at the next share or the next start.
     */
    internal const val KEEP_MS = 6 * 60 * 60 * 1000L

    /** This process's copies (being made, waiting for a project, in a composer). Anything else under the folder is a leftover. */
    private val live = HashSet<File>()

    /** The shared item, once, for the room it was sent to. */
    fun take(roomId: String): Shared? {
        if (target.value != roomId) return null
        val s = pending.value
        pending.value = null
        target.value = null
        return s
    }

    /** A new share is waiting for its project. One that never got there makes way (and its file goes). */
    fun offer(shared: Shared) {
        discard()
        pending.value = shared
    }

    /** The share that's waiting is over without being sent (cancelled, backed out of, replaced). */
    fun discard() {
        val s = pending.value
        pending.value = null
        target.value = null
        s?.file?.parentFile?.let(::delete)
    }

    /**
     * Text from the quick prompt, for [roomId]'s composer to take as it
     * takes a share. Not while a share waits at "Send to…": that one is
     * somebody's file, and it isn't replaced (false).
     */
    fun handOver(roomId: String, text: String): Boolean {
        if (pending.value != null && target.value == null) return false
        discard()
        // The target first: a composer already on screen takes it the moment `pending` changes.
        target.value = roomId
        pending.value = Shared(text, null)
        return true
    }

    /**
     * Whether a shared stream may be read at all. Only `content:` — a
     * `file:` path we can open is by definition one of our own private
     * files — and never our own provider: another app can't hold a grant to
     * it, so a share naming it is asking us to hand over our cache.
     */
    internal fun acceptable(scheme: String?, authority: String?, ownAuthority: String): Boolean =
        scheme == "content" && !authority.isNullOrEmpty() && authority != ownAuthority

    /**
     * An ACTION_SEND intent as a [Shared], or null when there is nothing in
     * it to send. The file is copied into our cache at once: the read grant
     * that came with the intent doesn't outlive it. [onProgress] follows the
     * copy; cancelling the coroutine stops it and removes the partial file.
     */
    suspend fun fromIntent(context: Context, intent: Intent, onProgress: (ShareCopy) -> Unit = {}): Shared? = withContext(Dispatchers.IO) {
        if (intent.action != Intent.ACTION_SEND) return@withContext null
        val text = listOfNotNull(
            intent.getStringExtra(Intent.EXTRA_SUBJECT),
            intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString(),
        ).filter { it.isNotBlank() }.distinct().joinToString("\n").ifBlank { null }
        @Suppress("DEPRECATION")
        val uri: Uri? = if (android.os.Build.VERSION.SDK_INT >= 33) intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
        else intent.getParcelableExtra(Intent.EXTRA_STREAM)
        var problem: String? = null
        var copy: File? = null
        val attachment = uri?.let { u ->
            if (!acceptable(u.scheme, u.authority, context.packageName + ".files")) {
                Diag.error("A share pointed at ${u.scheme}://${u.authority.orEmpty()}: not read")
                problem = "That file can't be shared from there."
                return@let null
            }
            val dir = claim(File(context.cacheDir, "$SHARED_DIR/${UUID.randomUUID()}"))
            try {
                val a = Uploads.describe(context, u)
                dir.mkdirs()
                val file = File(dir, Uploads.safeName(a.name))
                context.contentResolver.openInputStream(u)?.use { input ->
                    file.outputStream().use { out ->
                        val buffer = ByteArray(64 * 1024)
                        var copied = 0L
                        onProgress(ShareCopy(a.name, 0, a.size))
                        while (true) {
                            ensureActive()
                            val n = input.read(buffer)
                            if (n < 0) break
                            out.write(buffer, 0, n)
                            copied += n
                            onProgress(ShareCopy(a.name, copied, a.size))
                        }
                    }
                } ?: error("unreadable")
                copy = file
                a.copy(uri = androidx.core.content.FileProvider.getUriForFile(context, context.packageName + ".files", file), size = file.length())
            } catch (e: CancellationException) {
                delete(dir)
                throw e
            } catch (e: Exception) {
                Diag.error("Reading a shared file", e)
                delete(dir)
                problem = "The shared file couldn't be read."
                null
            }
        }
        if (text == null && attachment == null && problem == null) return@withContext null
        Shared(text, attachment, problem, copy)
    }

    /** [dir] is a copy this process is making (or made): not a leftover. */
    internal fun claim(dir: File): File = synchronized(live) {
        live += dir
        dir
    }

    private fun delete(dir: File) = synchronized(live) {
        live -= dir
        if (dir.exists() && !dir.deleteRecursively()) Diag.error("Removing a shared file's copy (${dir.name})")
    }

    /** Remove copies nothing can still want: leftovers of an earlier process, and ones taken long ago. */
    fun sweep(context: Context) = sweep(File(context.cacheDir, SHARED_DIR), System.currentTimeMillis())

    internal fun sweep(root: File, now: Long, keepMs: Long = KEEP_MS) {
        val dirs = root.listFiles() ?: return
        for (dir in dirs) {
            val waiting = pending.value?.file?.parentFile == dir
            val wanted = synchronized(live) { dir in live } && (waiting || now - dir.lastModified() <= keepMs)
            if (!wanted) delete(dir)
        }
    }
}

/**
 * What the (otherwise invisible) share activity shows while a big file is
 * copied in: without it the phone looked frozen — a transparent activity
 * swallowing every touch until the copy was done. Nothing for the first
 * moment, so a link or a small file doesn't flash a card.
 */
@Composable
fun ShareProgress(progress: StateFlow<ShareCopy?>, onCancel: () -> Unit) {
    val copy by progress.collectAsState()
    var slow by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        delay(400)
        slow = true
    }
    BackHandler(onBack = onCancel)
    val c = copy
    if (!slow || c == null) return
    Box(Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.32f)), contentAlignment = Alignment.Center) {
        Surface(
            shape = RoundedCornerShape(28.dp),
            color = MaterialTheme.colorScheme.surfaceContainerHigh,
            modifier = Modifier.padding(24.dp).widthIn(max = 360.dp),
        ) {
            Column(Modifier.padding(start = 24.dp, end = 24.dp, top = 24.dp, bottom = 12.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text("Getting it ready…", style = MaterialTheme.typography.titleMedium)
                Text(c.name, style = MaterialTheme.typography.bodyMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                val total = c.total?.takeIf { it > 0 }
                if (total != null) {
                    LinearProgressIndicator(progress = { (c.copied.toFloat() / total).coerceIn(0f, 1f) }, modifier = Modifier.fillMaxWidth())
                } else {
                    LinearProgressIndicator(Modifier.fillMaxWidth())
                }
                Text(
                    if (total != null) "${megabytes(c.copied)} of ${megabytes(total)} MB" else "${megabytes(c.copied)} MB",
                    style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                TextButton(onClick = onCancel, modifier = Modifier.align(Alignment.End)) { Text("Cancel") }
            }
        }
    }
}

private fun megabytes(bytes: Long): String = String.format(java.util.Locale.US, "%.1f", bytes / 1_000_000.0)

/** "Send to…": the agent rooms, for something shared from another app. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ShareScreen(back: () -> Unit, swap: (Screen) -> Unit) {
    val shared by ShareInbox.pending.collectAsState()
    val rooms by Matrix.agentRooms.collectAsState()
    val now by rememberNow(60_000)
    val sorted = rooms.values.sortedWith(AGENT_ROOM_ORDER)
    // Leaving without picking a project (Back, a notification opening
    // something else) ends the share: its file would otherwise sit in the
    // cache, and the item in the inbox, with no screen left to send it from.
    DisposableEffect(Unit) {
        onDispose { if (ShareInbox.target.value == null) ShareInbox.discard() }
    }
    Scaffold(topBar = {
        TopAppBar(
            navigationIcon = { IconButton(onClick = { ShareInbox.discard(); back() }) { Icon(Icons.Filled.Close, "Cancel") } },
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
                            // The text came, its file didn't: say so before it's sent without it.
                            s.problem?.let { Text("$it Only the text will be sent.", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error) }
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
                    Modifier.fillMaxWidth().clickable(role = Role.Button) {
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
                            listOfNotNull(if (r.isDm) "Agent" else "Project", live?.state?.label).joinToString(" · "),
                            style = MaterialTheme.typography.bodySmall, color = live?.state?.color() ?: MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    r.last?.let { Text(elapsed(it.ts, now), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
                }
            }
        }
    }
}
