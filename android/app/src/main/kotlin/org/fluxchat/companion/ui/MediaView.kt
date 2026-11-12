package org.fluxchat.companion.ui

import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.rememberTransformableState
import androidx.compose.foundation.gestures.transformable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.outlined.BrokenImage
import androidx.compose.material.icons.outlined.Download
import androidx.compose.material.icons.outlined.Share
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.toSize
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.fluxchat.companion.Diag
import org.fluxchat.companion.agents.Media
import org.fluxchat.companion.matrix.Matrix
import org.matrix.rustcomponents.sdk.MediaSource

/** Message images: downloaded through the SDK (decrypted in encrypted rooms), decoded at a bounded size. */
internal object Images {
    private const val THUMB_PX = 800UL
    private const val FULL_MAX_PX = 2560
    private val cache = object : LruCache<String, ImageBitmap>(48 * 1024 * 1024) {
        override fun sizeOf(key: String, value: ImageBitmap) = value.width * value.height * 4
    }

    /** Images whose failure is already in the diagnostics: a list of them offline would fill the log on every scroll. */
    private val reported = HashSet<String>()

    fun cached(key: String): ImageBitmap? = cache.get(key)

    suspend fun load(media: Media, full: Boolean): ImageBitmap? {
        val key = (if (full) "full:" else "thumb:") + media.source
        cache.get(key)?.let { return it }
        val client = Matrix.client ?: return null
        return withContext(Dispatchers.IO) {
            try {
                // Encrypted files can't be thumbnailed by the server: fetch them whole.
                val encrypted = media.source.contains("\"file\"")
                val bytes = when {
                    !full && media.thumbnail != null -> MediaSource.fromJson(media.thumbnail).use { client.getMediaContent(it) }
                    !full && !encrypted -> MediaSource.fromJson(media.source).use { client.getMediaThumbnail(it, THUMB_PX, THUMB_PX) }
                    else -> MediaSource.fromJson(media.source).use { client.getMediaContent(it) }
                }
                decode(bytes, if (full) FULL_MAX_PX else THUMB_PX.toInt()).also {
                    if (it == null) report(key, "Decoding an image (${bytes.size} bytes): not a picture this phone reads", null)
                }
            } catch (e: kotlinx.coroutines.CancellationException) {
                // Scrolled away mid-download is not a failure.
                throw e
            } catch (e: Throwable) {
                report(key, "Loading an image", e)
                null
            }
        }?.also {
            cache.put(key, it)
            synchronized(reported) { reported -= key }
        }
    }

    private fun report(key: String, what: String, e: Throwable?) {
        val first = synchronized(reported) {
            if (reported.size > 500) reported.clear()
            reported.add(key)
        }
        if (first) Diag.error(what, e)
    }

    private fun decode(bytes: ByteArray, maxPx: Int): ImageBitmap? {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
        var sample = 1
        while (bounds.outWidth / (sample * 2) >= maxPx || bounds.outHeight / (sample * 2) >= maxPx) sample *= 2
        val opts = BitmapFactory.Options().apply { inSampleSize = sample }
        return BitmapFactory.decodeByteArray(bytes, 0, bytes.size, opts)?.asImageBitmap()
    }
}

/** How far in a viewer zooms: pinch up to this, double-tap to [DOUBLE_TAP_ZOOM]. */
internal const val MAX_ZOOM = 6f
private const val DOUBLE_TAP_ZOOM = 2.5f

/** The size an image of [image] pixels is drawn at when fitted inside [viewport] (`ContentScale.Fit`). */
internal fun fittedSize(image: Size, viewport: Size): Size {
    if (image.width <= 0f || image.height <= 0f || viewport.width <= 0f || viewport.height <= 0f) return Size.Zero
    val scale = minOf(viewport.width / image.width, viewport.height / image.height)
    return Size(image.width * scale, image.height * scale)
}

/**
 * [pan] held to where the zoomed image still covers the screen: on each axis
 * it may move by half of what sticks out past the viewport, and not at all
 * while it fits. Unbounded, a zoomed image could be flung off screen, leaving
 * a black viewer with nothing to grab.
 */
internal fun clampPan(pan: Offset, scale: Float, image: Size, viewport: Size): Offset {
    val fitted = fittedSize(image, viewport)
    val maxX = (fitted.width * scale - viewport.width) / 2f
    val maxY = (fitted.height * scale - viewport.height) / 2f
    // Plain 0 where it fits (coerceIn(-0f, 0f) hands back a negative zero, which is not equal to Offset.Zero).
    return Offset(if (maxX > 0f) pan.x.coerceIn(-maxX, maxX) else 0f, if (maxY > 0f) pan.y.coerceIn(-maxY, maxY) else 0f)
}

/** An image in a message: sized from its info so the list doesn't jump; tap for full screen. */
@Composable
fun MessageImage(media: Media) {
    var viewing by remember { mutableStateOf(false) }
    var attempt by remember { mutableIntStateOf(0) }
    val state by produceState<Any?>(Images.cached("thumb:" + media.source), media, attempt) {
        // From the cache again, not from `value`: that is still the last
        // image's (or the last attempt's "failed") when this restarts.
        val cached = Images.cached("thumb:" + media.source)
        value = cached
        if (cached == null) value = Images.load(media, full = false) ?: false
    }
    val ratio = if ((media.width ?: 0) > 0 && (media.height ?: 0) > 0) media.width!!.toFloat() / media.height!! else 4f / 3f
    val failed = state == false
    Box(
        Modifier.fillMaxWidth().heightIn(max = 360.dp).aspectRatio(ratio.coerceIn(0.5f, 2.5f))
            .clip(RoundedCornerShape(16.dp))
            .background(MaterialTheme.colorScheme.surfaceContainerHighest)
            // A picture that didn't load is a tap away from another try (it
            // used to stay broken until the message left the screen and came back).
            .clickable(
                enabled = state is ImageBitmap || failed,
                onClickLabel = if (failed) "try again" else "view full screen",
            ) { if (failed) attempt++ else viewing = true },
        contentAlignment = Alignment.Center,
    ) {
        when (val s = state) {
            is ImageBitmap -> Image(s, "Image", Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
            false -> Icon(Icons.Outlined.BrokenImage, "Couldn't load the image", tint = MaterialTheme.colorScheme.onSurfaceVariant)
            else -> CircularProgressIndicator(Modifier.size(28.dp), strokeWidth = 2.dp)
        }
    }
    if (viewing) Viewer(media) { viewing = false }
}

/** Full screen: pinch or double-tap to zoom, save, share. */
@Composable
private fun Viewer(media: Media, close: () -> Unit) {
    val context = LocalContext.current
    ViewerDialog(close) {
        ViewerImage(media, "Image")
        ViewerBar(close) {
            val busy = fileBusy(media.source)
            ViewerButton(Icons.Outlined.Download, "Save the image", enabled = !busy) { FileJobs.saveImage(context, media) }
            ViewerButton(Icons.Outlined.Share, "Share the image", enabled = !busy) { FileJobs.shareImage(context, media) }
        }
    }
}

/** The black full-screen window the image viewers draw in. */
@Composable
internal fun ViewerDialog(close: () -> Unit, content: @Composable BoxScope.() -> Unit) {
    // Its own window, above the lock screen: not while the app is locked.
    if (!unlocked()) return
    Dialog(onDismissRequest = close, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        SavesHere()
        Box(Modifier.fillMaxSize().background(Color.Black), content = content)
    }
}

/** Close on one side, the viewer's actions on the other. */
@Composable
internal fun BoxScope.ViewerBar(close: () -> Unit, actions: @Composable RowScope.() -> Unit) {
    Row(
        Modifier.align(Alignment.TopCenter).fillMaxWidth().safeDrawingPadding().padding(8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        ViewerButton(Icons.Default.Close, "Close", onClick = close)
        Box(Modifier.weight(1f))
        actions()
    }
}

@Composable
internal fun ViewerButton(icon: ImageVector, label: String, enabled: Boolean = true, onClick: () -> Unit) {
    IconButton(
        onClick = onClick,
        enabled = enabled,
        colors = IconButtonDefaults.iconButtonColors(
            containerColor = Color.Black.copy(alpha = 0.5f), contentColor = Color.White,
            disabledContainerColor = Color.Black.copy(alpha = 0.5f), disabledContentColor = Color.White.copy(alpha = 0.4f),
        ),
    ) { Icon(icon, label) }
}

/**
 * The image itself, full size (its thumbnail while that loads): zoomable once
 * it is there, a spinner until then, and when it can't be had, a tap to try
 * again — both viewers used to spin for ever on a failed download.
 */
@Composable
internal fun ViewerImage(media: Media, description: String, onZoomed: (Boolean) -> Unit = {}) {
    key(media.source) {
        var attempt by remember { mutableIntStateOf(0) }
        val state by produceState<Any?>(Images.cached("full:" + media.source) ?: Images.cached("thumb:" + media.source), attempt) {
            val shown = Images.cached("full:" + media.source) ?: Images.cached("thumb:" + media.source)
            value = shown
            value = Images.load(media, full = true) ?: shown ?: Images.load(media, full = false) ?: false
        }
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            when (val s = state) {
                is ImageBitmap -> ZoomableImage(s, description, onZoomed)
                false -> Column(
                    Modifier.clip(RoundedCornerShape(16.dp)).clickable(onClickLabel = "try again", role = Role.Button) { attempt++ }.padding(24.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Icon(Icons.Outlined.BrokenImage, null, Modifier.size(40.dp), tint = Color.White.copy(alpha = 0.8f))
                    Text("Couldn't load the image.\nTap to try again.", color = Color.White, textAlign = TextAlign.Center)
                }
                else -> CircularProgressIndicator()
            }
        }
    }
}

/** Pinch to zoom, drag to look around (no further than the image's own edges), double-tap in and out. */
@Composable
private fun ZoomableImage(image: ImageBitmap, description: String, onZoomed: (Boolean) -> Unit) {
    var scale by remember(image) { mutableFloatStateOf(1f) }
    var offset by remember(image) { mutableStateOf(Offset.Zero) }
    var viewport by remember { mutableStateOf(IntSize.Zero) }
    val size = Size(image.width.toFloat(), image.height.toFloat())
    val transform = rememberTransformableState { _, zoom, pan, _ ->
        scale = (scale * zoom).coerceIn(1f, MAX_ZOOM)
        offset = clampPan(offset + pan, scale, size, viewport.toSize())
    }
    val zoomed = scale > 1f
    val report by rememberUpdatedState(onZoomed)
    LaunchedEffect(zoomed) { report(zoomed) }
    Image(
        image, description,
        Modifier.fillMaxSize()
            .onSizeChanged { viewport = it }
            .pointerInput(image) {
                detectTapGestures(onDoubleTap = {
                    scale = if (scale > 1f) 1f else DOUBLE_TAP_ZOOM
                    offset = Offset.Zero
                })
            }
            // At rest a one-finger drag isn't ours: a pager around the image gets its swipe.
            .transformable(transform, canPan = { scale > 1f })
            .graphicsLayer {
                scaleX = scale
                scaleY = scale
                translationX = offset.x
                translationY = offset.y
            },
        contentScale = ContentScale.Fit,
    )
}
