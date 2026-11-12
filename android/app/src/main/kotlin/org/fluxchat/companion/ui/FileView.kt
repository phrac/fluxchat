package org.fluxchat.companion.ui

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import android.text.format.Formatter
import android.widget.Toast
import androidx.activity.compose.LocalActivityResultRegistryOwner
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.ActivityResultRegistryOwner
import androidx.activity.result.contract.ActivityResultContract
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
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.withContext
import org.fluxchat.companion.AppLock
import org.fluxchat.companion.Diag
import org.fluxchat.companion.agents.FileKind
import org.fluxchat.companion.agents.FileRef
import org.fluxchat.companion.agents.Media
import org.fluxchat.companion.matrix.Files
import org.fluxchat.companion.matrix.Matrix
import org.fluxchat.companion.matrix.imageMime
import java.io.File
import java.lang.ref.WeakReference

/**
 * Opening, saving and sharing attachments — work that belongs to the app, not
 * to the card that was tapped. A card is an item of a lazy list: with the
 * save picker's launcher registered by the card, its answer was dropped
 * whenever the card had left the list meanwhile (a new message pushes it out
 * while the picker is open), and a download begun from the card stopped the
 * moment it scrolled away. Here the picker answers to the activity, what it
 * was asked for waits in [pending], and the work runs on [Matrix.launch].
 */
internal object FileJobs {
    /** What a save writes once the picker has said where. */
    private class Save(val key: String, val name: String, val file: suspend (Context) -> File)

    private val _busy = MutableStateFlow<Set<String>>(emptySet())

    /** The sources being downloaded, opened, saved or shared: a card shows its spinner whenever it is on screen. */
    val busy: StateFlow<Set<String>> = _busy

    /** The save the open picker is for (the system shows one picker at a time). Main thread only. */
    private var pending: Save? = null
    private var launcher: ActivityResultLauncher<SaveAs>? = null
    private var host: WeakReference<ActivityResultRegistryOwner>? = null
    private var app: Context? = null

    /**
     * Take the save picker's answers on this activity. Whatever can ask for a
     * save calls this when it appears; after the activity is recreated under
     * an open picker (the phone was turned), the first caller is who the
     * waiting answer is delivered through.
     */
    fun attach(activity: ActivityResultRegistryOwner, context: Context) {
        app = context.applicationContext
        if (host?.get() === activity) return
        launcher?.unregister()
        host = WeakReference(activity)
        launcher = activity.activityResultRegistry.register(PICKER_KEY, SaveContract) { picked(it) }
        (activity as? LifecycleOwner)?.lifecycle?.addObserver(object : DefaultLifecycleObserver {
            override fun onDestroy(owner: LifecycleOwner) {
                // Let go of the activity. Its registry keeps an answer that is
                // still on its way, for the next activity's attach.
                if (host?.get() === activity) {
                    launcher?.unregister()
                    launcher = null
                    host = null
                }
            }
        })
    }

    /** Download the file if need be, then hand it to an app that opens its type. */
    fun open(context: Context, f: FileRef) {
        val app = context.applicationContext
        job(app, f.source, "Open", f.name) {
            val file = Files.fetch(app, f)
            withContext(Dispatchers.Main) {
                if (!Files.open(app, file, f.mime)) say(app, "No app here opens ${f.mime ?: "this file"}. Save it instead.")
            }
        }
    }

    /** Ask where to save the file; it is downloaded (if need be) and written once that's answered. */
    fun save(context: Context, f: FileRef) =
        ask(context.applicationContext, Save(f.source, f.name) { Files.fetch(it, f) }, f.mime ?: "application/octet-stream")

    /** Save a message image: fetched first, since its name and type come from its bytes. */
    fun saveImage(context: Context, media: Media) {
        val app = context.applicationContext
        job(app, media.source, "Save", "the image") {
            val file = Files.fetchImage(app, media)
            withContext(Dispatchers.Main) { ask(app, Save(media.source, file.name) { file }, imageMime(file.name)) }
        }
    }

    /** Offer a message image to other apps. */
    fun shareImage(context: Context, media: Media) {
        val app = context.applicationContext
        job(app, media.source, "Share", "the image") {
            val file = Files.fetchImage(app, media)
            withContext(Dispatchers.Main) {
                if (!Files.share(app, file, imageMime(file.name))) say(app, "No app here takes images.")
            }
        }
    }

    /** Offer a file to other apps. */
    fun share(context: Context, f: FileRef) {
        val app = context.applicationContext
        job(app, f.source, "Share", f.name) {
            val file = Files.fetch(app, f)
            withContext(Dispatchers.Main) {
                if (!Files.share(app, file, f.mime)) say(app, "No app here takes ${f.mime ?: "this file"}.")
            }
        }
    }

    private fun ask(app: Context, save: Save, mime: String) {
        val picker = launcher
        if (picker == null) {
            Diag.error("Save ${save.name}: nothing to show the save dialog on", null)
            say(app, "Couldn't open the save dialog.")
            return
        }
        pending = save
        try {
            // Our own detour: coming back from the picker isn't a reason to lock.
            AppLock.hop()
            picker.launch(SaveAs(save.name, mime))
        } catch (e: ActivityNotFoundException) {
            pending = null
            Diag.error("Save ${save.name}: no app to pick where", e)
            say(app, "No app on this phone can pick where to save.")
        }
    }

    private fun picked(target: Uri?) {
        val save = pending
        pending = null
        val app = app
        // Backed out of the picker: nothing was asked for after all.
        if (target == null || app == null) return
        if (save == null) {
            // The app was restarted under the picker: what it was for is gone,
            // and the picker has already created the (empty) document.
            Diag.error("A place to save came back with nothing waiting for it", null)
            runCatching { DocumentsContract.deleteDocument(app.contentResolver, target) }
                .onFailure { Diag.error("Removing the empty document", it) }
            say(app, "Couldn't save. Try again.")
            return
        }
        job(app, save.key, "Save", save.name) {
            Files.saveTo(app, save.file(app), target)
            withContext(Dispatchers.Main) { say(app, "Saved ${save.name}", long = false) }
        }
    }

    /** Run [block] for the file [key] unless something already is: a second tap is the same request. */
    private fun job(app: Context, key: String, verb: String, name: String, block: suspend () -> Unit) {
        var mine = false
        _busy.update { mine = key !in it; it + key }
        if (!mine) return
        Matrix.launch {
            try {
                block()
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Throwable) {
                Diag.error("$verb $name", e)
                withContext(Dispatchers.Main) { say(app, "Couldn't ${verb.lowercase()} $name: ${e.message ?: e.javaClass.simpleName}") }
            } finally {
                _busy.update { it - key }
            }
        }
    }

    private fun say(app: Context, text: String, long: Boolean = true) =
        Toast.makeText(app, text, if (long) Toast.LENGTH_LONG else Toast.LENGTH_SHORT).show()

    private const val PICKER_KEY = "fluxchat.save"
}

/** A name and type for the document the save picker creates. */
private class SaveAs(val name: String, val mime: String)

/** `ActivityResultContracts.CreateDocument`, with the type given per launch (one registration serves every file). */
private object SaveContract : ActivityResultContract<SaveAs, Uri?>() {
    override fun createIntent(context: Context, input: SaveAs): Intent =
        Intent(Intent.ACTION_CREATE_DOCUMENT).setType(input.mime).putExtra(Intent.EXTRA_TITLE, input.name)

    override fun parseResult(resultCode: Int, intent: Intent?): Uri? =
        intent?.takeIf { resultCode == Activity.RESULT_OK }?.data
}

/** Whatever offers Save has the picker's answers taken on its activity (see [FileJobs.attach]). */
@Composable
internal fun SavesHere() {
    val owner = LocalActivityResultRegistryOwner.current
    val context = LocalContext.current
    DisposableEffect(owner) {
        owner?.let { FileJobs.attach(it, context) }
        onDispose {}
    }
}

/** Whether something is being done with the file or image [source] (see [FileJobs.busy]). */
@Composable
internal fun fileBusy(source: String): Boolean {
    val flow = remember(source) { FileJobs.busy.map { source in it }.distinctUntilChanged() }
    val busy by flow.collectAsState(source in FileJobs.busy.value)
    return busy
}

/**
 * A file in a message: name, size and type; tap to open it in another app,
 * the download button to save it. A video shows its thumbnail.
 */
@Composable
fun FileCard(f: FileRef, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    SavesHere()
    val busy = fileBusy(f.source)
    val scheme = MaterialTheme.colorScheme
    Surface(color = scheme.surfaceContainerHighest, shape = RoundedCornerShape(14.dp), onClick = { FileJobs.open(context, f) }, modifier = modifier.fillMaxWidth()) {
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
                IconButton(onClick = { FileJobs.save(context, f) }, enabled = !busy) { Icon(Icons.Outlined.Download, "Save ${f.name}") }
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
