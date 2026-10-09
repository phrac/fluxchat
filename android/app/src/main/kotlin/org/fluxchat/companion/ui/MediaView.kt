package org.fluxchat.companion.ui

import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.rememberTransformableState
import androidx.compose.foundation.gestures.transformable
import androidx.compose.foundation.layout.Box
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
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.fluxchat.companion.agents.Media
import org.fluxchat.companion.matrix.Matrix
import org.matrix.rustcomponents.sdk.MediaSource

/** Message images: downloaded through the SDK (decrypted in encrypted rooms), decoded at a bounded size. */
private object Images {
    private const val THUMB_PX = 800UL
    private const val FULL_MAX_PX = 2560
    private val cache = object : LruCache<String, ImageBitmap>(48 * 1024 * 1024) {
        override fun sizeOf(key: String, value: ImageBitmap) = value.width * value.height * 4
    }

    fun cached(key: String): ImageBitmap? = cache.get(key)

    suspend fun load(media: Media, full: Boolean): ImageBitmap? {
        val key = (if (full) "full:" else "thumb:") + media.source
        cache.get(key)?.let { return it }
        val client = Matrix.client ?: return null
        return withContext(Dispatchers.IO) {
            runCatching {
                // Encrypted files can't be thumbnailed by the server: fetch them whole.
                val encrypted = media.source.contains("\"file\"")
                val bytes = when {
                    !full && media.thumbnail != null -> MediaSource.fromJson(media.thumbnail).use { client.getMediaContent(it) }
                    !full && !encrypted -> MediaSource.fromJson(media.source).use { client.getMediaThumbnail(it, THUMB_PX, THUMB_PX) }
                    else -> MediaSource.fromJson(media.source).use { client.getMediaContent(it) }
                }
                decode(bytes, if (full) FULL_MAX_PX else THUMB_PX.toInt())
            }.getOrNull()
        }?.also { cache.put(key, it) }
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

/** An image in a message: sized from its info so the list doesn't jump; tap for full screen. */
@Composable
fun MessageImage(media: Media) {
    var viewing by remember { mutableStateOf(false) }
    val state by produceState<Any?>(Images.cached("thumb:" + media.source), media) {
        if (value == null) value = Images.load(media, full = false) ?: false
    }
    val ratio = if ((media.width ?: 0) > 0 && (media.height ?: 0) > 0) media.width!!.toFloat() / media.height!! else 4f / 3f
    Box(
        Modifier.fillMaxWidth().heightIn(max = 360.dp).aspectRatio(ratio.coerceIn(0.5f, 2.5f))
            .clip(RoundedCornerShape(16.dp))
            .background(MaterialTheme.colorScheme.surfaceContainerHighest)
            .clickable(enabled = state is ImageBitmap) { viewing = true },
        contentAlignment = Alignment.Center,
    ) {
        when (val s = state) {
            is ImageBitmap -> Image(s, null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
            false -> Icon(Icons.Outlined.BrokenImage, "Couldn't load the image", tint = MaterialTheme.colorScheme.onSurfaceVariant)
            else -> CircularProgressIndicator(Modifier.size(28.dp), strokeWidth = 2.dp)
        }
    }
    if (viewing) Viewer(media) { viewing = false }
}

/** Full screen, pinch to zoom. */
@Composable
private fun Viewer(media: Media, close: () -> Unit) {
    val full by produceState(Images.cached("full:" + media.source) ?: Images.cached("thumb:" + media.source), media) {
        Images.load(media, full = true)?.let { value = it }
    }
    var scale by remember { mutableFloatStateOf(1f) }
    var offset by remember { mutableStateOf(androidx.compose.ui.geometry.Offset.Zero) }
    val transform = rememberTransformableState { _, zoom, pan, _ ->
        scale = (scale * zoom).coerceIn(1f, 6f)
        offset = if (scale == 1f) androidx.compose.ui.geometry.Offset.Zero else offset + pan
    }
    Dialog(onDismissRequest = close, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Box(Modifier.fillMaxSize().background(Color.Black)) {
            full?.let {
                Image(
                    it, null,
                    Modifier.fillMaxSize().transformable(transform).graphicsLayer {
                        scaleX = scale
                        scaleY = scale
                        translationX = offset.x
                        translationY = offset.y
                    },
                    contentScale = ContentScale.Fit,
                )
            } ?: CircularProgressIndicator(Modifier.align(Alignment.Center))
            IconButton(
                onClick = close,
                modifier = Modifier.safeDrawingPadding().padding(8.dp).align(Alignment.TopStart),
                colors = IconButtonDefaults.iconButtonColors(containerColor = Color.Black.copy(alpha = 0.5f), contentColor = Color.White),
            ) { Icon(Icons.Default.Close, "Close") }
        }
    }
}
