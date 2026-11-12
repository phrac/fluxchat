package org.fluxchat.companion.ui

import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.fluxchat.companion.Diag
import org.fluxchat.companion.agents.RunState
import org.fluxchat.companion.matrix.Matrix
import org.matrix.rustcomponents.sdk.MediaSource

/** Avatar thumbnails, decoded once (mxc URL → bitmap). */
private object Avatars {
    private const val THUMB_PX = 96UL
    private val cache = LruCache<String, ImageBitmap>(200)

    /** When each picture last failed to load. Tried again after [RETRY_MS]: "failed"
     * used to be for good, so one look at the app offline left initials until it was restarted. */
    private val failed = HashMap<String, Long>()
    private const val RETRY_MS = 60_000L

    fun cached(url: String): ImageBitmap? = cache.get(url)

    suspend fun load(url: String): ImageBitmap? {
        cache.get(url)?.let { return it }
        val before = synchronized(failed) { failed[url] }
        if (before != null && System.currentTimeMillis() - before < RETRY_MS) return null
        val client = Matrix.client ?: return null
        return withContext(Dispatchers.IO) {
            try {
                val bytes = MediaSource.fromUrl(url).use { client.getMediaThumbnail(it, THUMB_PX, THUMB_PX) }
                BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.asImageBitmap()
            } catch (e: kotlinx.coroutines.CancellationException) {
                // Scrolled away mid-download: not a failure to remember.
                throw e
            } catch (e: Throwable) {
                // Once per picture: a list of them offline would fill the log.
                if (before == null) Diag.error("Loading an avatar", e)
                null
            }
        }.also {
            if (it != null) cache.put(url, it)
            synchronized(failed) { if (it != null) failed -= url else failed[url] = System.currentTimeMillis() }
        }
    }
}

/**
 * A person (round) or room (rounded square, the desktop's shapes): the
 * picture when there is one, else a tinted initial.
 */
@Composable
fun Avatar(id: String, name: String, url: String?, size: Dp = 40.dp, room: Boolean = false, state: RunState? = null) {
    val shape: Shape = if (room) RoundedCornerShape(size * 0.28f) else CircleShape
    val image by produceState(url?.let { Avatars.cached(it) }, url) { if (url != null && value == null) value = Avatars.load(url) }
    Box(Modifier.size(size)) {
        val tint = nickColor(id)
        val img = image
        if (img != null) {
            Image(img, null, Modifier.size(size).clip(shape), contentScale = ContentScale.Crop)
        } else {
            Box(Modifier.size(size).background(tint.copy(alpha = 0.18f), shape), contentAlignment = Alignment.Center) {
                Text(
                    initial(name.ifBlank { id }),
                    color = tint,
                    fontWeight = FontWeight.SemiBold,
                    fontSize = (size.value * 0.42f).sp,
                )
            }
        }
        if (state != null) {
            val dot = size * 0.32f
            Box(
                Modifier.align(Alignment.BottomEnd).size(dot)
                    .border(2.dp, MaterialTheme.colorScheme.surface, CircleShape)
                    .background(state.color(), CircleShape),
            )
        }
    }
}

private fun initial(name: String): String {
    val n = name.trimStart('@', '#', '!').ifEmpty { return "?" }
    val cp = n.codePointAt(0)
    return String(Character.toChars(cp)).uppercase()
}
