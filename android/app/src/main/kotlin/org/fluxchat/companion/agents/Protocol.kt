package org.fluxchat.companion.agents

import org.json.JSONObject

/**
 * The agent protocol (docs/agent-protocol.md): `org.fluxchat.agent` in a
 * message's content — the latest edit's `m.new_content` first. When present
 * it wins over a harness's conventions.
 */
data class Protocol(
    val run: String? = null,
    val state: String? = null,
    val step: String? = null,
    val steps: Int? = null,
    val progress: Float? = null,
    val model: String? = null,
    val context: Float? = null,
    val cwd: String? = null,
    val costUsd: Double? = null,
    val choices: List<ProtocolChoice> = emptyList(),
) {
    fun runState(): RunState? = when (state) {
        "working" -> RunState.Working
        "waiting" -> RunState.NeedsYou
        "done" -> RunState.Done
        "failed" -> RunState.Failed
        else -> null
    }

    companion object {
        /** Unstable key, then the stable one an MSC would give it. */
        private val KEYS = listOf("org.fluxchat.agent", "m.agent")

        /** From an event's JSON (null when it has none). */
        fun fromEventJson(json: String?): Protocol? {
            if (json == null || KEYS.none { json.contains(it) }) return null
            val content = runCatching { JSONObject(json).optJSONObject("content") }.getOrNull() ?: return null
            val newContent = content.optJSONObject("m.new_content")
            for (key in KEYS) {
                val obj = newContent?.optJSONObject(key) ?: content.optJSONObject(key)
                if (obj != null) return fromJson(obj)
            }
            return null
        }

        fun fromJson(o: JSONObject): Protocol {
            fun str(k: String) = if (o.has(k) && !o.isNull(k)) o.optString(k) else null
            fun num(k: String) = if (o.has(k) && !o.isNull(k)) o.optDouble(k).takeIf { !it.isNaN() } else null
            val choices = o.optJSONArray("choices")?.let { arr ->
                (0 until arr.length()).mapNotNull { i ->
                    val c = arr.optJSONObject(i) ?: return@mapNotNull null
                    val key = c.optString("key").takeIf { it.isNotEmpty() } ?: return@mapNotNull null
                    ProtocolChoice(key, c.optString("label").ifEmpty { key })
                }
            } ?: emptyList()
            return Protocol(
                run = str("run"),
                state = str("state"),
                step = str("step"),
                steps = num("steps")?.toInt(),
                progress = num("progress")?.toFloat(),
                model = str("model"),
                context = num("context")?.toFloat(),
                cwd = str("cwd"),
                costUsd = num("cost_usd"),
                choices = choices,
            )
        }
    }
}

data class ProtocolChoice(val key: String, val label: String)
