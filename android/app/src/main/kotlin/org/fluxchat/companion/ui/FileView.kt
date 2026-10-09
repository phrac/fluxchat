package org.fluxchat.companion.ui

import android.text.format.Formatter
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.outlined.AudioFile
import androidx.compose.material.icons.outlined.Description
import androidx.compose.material.icons.outlined.Download
import androidx.compose.material.icons.outlined.Movie
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.fluxchat.companion.Diag
import org.fluxchat.companion.agents.FileKind
import org.fluxchat.companion.agents.FileRef
import org.fluxchat.companion.matrix.Files

/**
 * A file in a message: name, size and type; tap to open it in another app,
 * the download button to save it. A video shows its thumbnail.
 */
@Composable
fun FileCard(f: FileRef, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var busy by remember { mutableStateOf(false) }
    val fail: (String, Throwable) -> Unit = { what, e ->
        Diag.error("$what ${f.name}", e)
        Toast.makeText(context, "Couldn't ${what.lowercase()} ${f.name}: ${e.message ?: e.javaClass.simpleName}", Toast.LENGTH_LONG).show()
    }
    val save = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument(f.mime ?: "application/octet-stream")) { target ->
        if (target != null) scope.launch {
            busy = true
            runCatching { Files.saveTo(context, Files.fetch(context, f), target) }
                .onSuccess { Toast.makeText(context, "Saved ${f.name}", Toast.LENGTH_SHORT).show() }
                .onFailure { fail("Save", it) }
            busy = false
        }
    }
    val open: () -> Unit = {
        if (!busy) scope.launch {
            busy = true
            runCatching { Files.fetch(context, f) }
                .onSuccess { file ->
                    if (!Files.open(context, file, f.mime)) Toast.makeText(context, "No app here opens ${f.mime ?: "this file"}. Save it instead.", Toast.LENGTH_LONG).show()
                }
                .onFailure { fail("Open", it) }
            busy = false
        }
    }
    val scheme = MaterialTheme.colorScheme
    Surface(color = scheme.surfaceContainerHighest, shape = RoundedCornerShape(14.dp), onClick = open, modifier = modifier.fillMaxWidth()) {
        Column {
            if (f.kind == FileKind.Video && f.thumbnail != null) VideoThumb(f)
            Row(Modifier.padding(start = 10.dp, top = 8.dp, bottom = 8.dp, end = 2.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(40.dp).background(scheme.secondaryContainer, RoundedCornerShape(10.dp)), contentAlignment = Alignment.Center) {
                    if (busy) {
                        CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
                    } else {
                        Icon(
                            when (f.kind) {
                                FileKind.Video -> Icons.Outlined.Movie
                                FileKind.Audio -> Icons.Outlined.AudioFile
                                FileKind.File -> Icons.Outlined.Description
                            },
                            null, tint = scheme.onSecondaryContainer,
                        )
                    }
                }
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(f.name, style = MaterialTheme.typography.bodyMedium, maxLines = 2, overflow = TextOverflow.Ellipsis)
                    Text(
                        listOfNotNull(
                            f.size?.let { Formatter.formatShortFileSize(context, it) },
                            f.durationMs?.let { duration(it) },
                            typeLabel(f),
                        ).joinToString(" · "),
                        style = MaterialTheme.typography.labelSmall, color = scheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis,
                    )
                }
                IconButton(onClick = { save.launch(f.name) }, enabled = !busy) { Icon(Icons.Outlined.Download, "Save ${f.name}") }
            }
        }
    }
}

@Composable
private fun VideoThumb(f: FileRef) {
    val media = f.thumbnail ?: return
    val thumb by produceState<ImageBitmap?>(Images.cached("thumb:" + media.source), media) {
        if (value == null) value = Images.load(media, full = false)
    }
    val ratio = if ((media.width ?: 0) > 0 && (media.height ?: 0) > 0) media.width!!.toFloat() / media.height!! else 16f / 9f
    Box(
        Modifier.fillMaxWidth().heightIn(max = 280.dp).aspectRatio(ratio.coerceIn(0.5f, 2.5f))
            .clip(RoundedCornerShape(topStart = 14.dp, topEnd = 14.dp)).background(MaterialTheme.colorScheme.surfaceContainer),
        contentAlignment = Alignment.Center,
    ) {
        thumb?.let { Image(it, null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop) }
        Box(Modifier.size(52.dp).background(Color.Black.copy(alpha = 0.55f), CircleShape), contentAlignment = Alignment.Center) {
            Icon(Icons.Filled.PlayArrow, "Play", tint = Color.White, modifier = Modifier.size(32.dp))
        }
    }
}

/** "PDF", "MP4"…: the file's extension, else its type's subtype. */
private fun typeLabel(f: FileRef): String? =
    f.name.substringAfterLast('.', "").takeIf { it.isNotEmpty() && it.length <= 5 && it != f.name }?.uppercase()
        ?: f.mime?.substringAfter('/')?.takeIf { it.isNotEmpty() && it != "octet-stream" }?.uppercase()

private fun duration(ms: Long): String {
    val s = ms / 1000
    return if (s >= 3600) "%d:%02d:%02d".format(s / 3600, (s % 3600) / 60, s % 60) else "%d:%02d".format(s / 60, s % 60)
}
