package org.fluxchat.companion.matrix

import org.json.JSONArray
import org.json.JSONObject

/**
 * Pinned sessions. A pin keeps a session at the top of its project's session
 * list and keeps the project on the Projects screen as newer work arrives.
 * Stored as global account data (per-user, syncs across devices, shared with
 * the desktop client), like [AgentList].
 */
data class Pin(val roomId: String, val root: String, val pinnedAt: Long)

/** One global account data event: `org.fluxchat.pins`. */
object PinStore {
    const val TYPE = "org.fluxchat.pins"

    data class Pins(val pins: List<Pin> = emptyList()) {
        /** Is this session pinned? */
        fun isPinned(roomId: String, root: String): Boolean = pins.any { it.roomId == roomId && it.root == root }

        /** Does this project hold any pinned session? */
        fun hasRoom(roomId: String): Boolean = pins.any { it.roomId == roomId }

        /** When the project's oldest pin was made (its row's spot among pins), else none. */
        fun roomPinnedAt(roomId: String): Long? = pins.filter { it.roomId == roomId }.minOfOrNull { it.pinnedAt }

        /** Keep only pins for projects you're in: a left project's pins go. */
        fun prune(rooms: Set<String>): Pins = Pins(pins.filter { it.roomId in rooms })

        fun toJson(): String = JSONObject().apply {
            put("pins", JSONArray(pins.map { p ->
                JSONObject().put("room_id", p.roomId).put("root", p.root).put("pinned_at", p.pinnedAt)
            }))
        }.toString()
    }

    /** Tolerant parse (the desktop may write this too): garbage is no pins. */
    fun parse(json: String?): Pins {
        val o = json?.let { runCatching { JSONObject(it) }.getOrNull() } ?: return Pins()
        val arr = o.optJSONArray("pins") ?: return Pins()
        val pins = (0 until arr.length()).mapNotNull { i ->
            val p = arr.optJSONObject(i) ?: return@mapNotNull null
            val roomId = p.optString("room_id").takeIf { it.startsWith("!") } ?: return@mapNotNull null
            val root = p.optString("root").takeIf { it.startsWith("$") } ?: return@mapNotNull null
            Pin(roomId, root, p.optLong("pinned_at", 0L))
        }
        // A same session pinned twice (two devices racing): newest wins.
        val byKey = LinkedHashMap<String, Pin>()
        for (p in pins) {
            val k = p.roomId + "|" + p.root
            val old = byKey[k]
            if (old == null || p.pinnedAt >= old.pinnedAt) byKey[k] = p
        }
        return Pins(byKey.values.sortedWith(compareByDescending<Pin> { it.pinnedAt }))
    }
}

/**
 * Pinned rows first, everything else as it came (already newest-activity
 * first). A stable sort keeps the pins in the list's own recency order:
 * pinning never re-sorts the rows under it.
 */
fun <T> pinnedFirst(items: List<T>, isPinned: (T) -> Boolean): List<T> =
    items.sortedByDescending { if (isPinned(it)) 1 else 0 }
