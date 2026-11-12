package org.fluxchat.companion.ui

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import org.fluxchat.companion.agents.BusyMode
import org.fluxchat.companion.agents.ViewKey
import org.fluxchat.companion.matrix.Attachment

/**
 * What you sent that could not even be handed over (neither to the send
 * queue nor straight to the server), per view, until its composer takes it
 * back. The composer is cleared the moment you tap Send, and such a failure
 * used to be a line in Diagnostics: the text was simply gone. Process-wide,
 * because the send outlives the screen — come back to the session and it's
 * there. (A send the queue did take shows its own failure, on its row.)
 */
object Unsent {
    /** As typed ([text]: the message, or the attachment's caption), to go through the composer's send again. */
    data class Item(val text: String, val mode: BusyMode, val attachment: Attachment? = null)

    private val items = MutableStateFlow<Map<ViewKey, List<Item>>>(emptyMap())
    val all: StateFlow<Map<ViewKey, List<Item>>> = items

    fun add(view: ViewKey, item: Item) = items.update { it + (view to it[view].orEmpty() + item) }

    /** [item] is back in the composer, sent again, or discarded. */
    fun remove(view: ViewKey, item: Item) = items.update { m ->
        val left = m[view].orEmpty() - item
        if (left.isEmpty()) m - view else m + (view to left)
    }

    fun clear() {
        items.value = emptyMap()
    }
}
