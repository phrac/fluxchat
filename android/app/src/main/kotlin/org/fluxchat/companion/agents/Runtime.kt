package org.fluxchat.companion.agents

import java.util.Locale

/**
 * The agent's runtime info (the desktop's agents.rs `Runtime`): from the
 * agent protocol's fields (`Protocol::runtime` there) or Hermes's footer
 * (`/footer on`) — the last paragraph of a final answer, `model · 42% ·
 * ~/dir` (any of those, plus latency like `22s`, or `alias → served`). The
 * newest per room backfills a finished run record's model and cost, the
 * desktop's `record_run` pulling `agent_runtime` when the run ends.
 */
data class Runtime(
    val model: String? = null,
    /** Context window used, whole percent (protocol `context` 0.0–1.0). */
    val contextPct: Int? = null,
    val cwd: String? = null,
    val latency: String? = null,
    /** Spend so far, from the protocol's `cost_usd` (the footer has no money part). */
    val costUsd: Double? = null,
) {
    /** "claude-opus-4 · ctx 42% · $0.18 · ~/proj" (the desktop's `summary`). */
    fun summary(): String = listOfNotNull(
        model,
        contextPct?.let { "ctx $it%" },
        costUsd?.let { "\$" + String.format(Locale.US, "%.2f", it) },
        cwd,
    ).joinToString(" · ")

    companion object {
        /**
         * Split Hermes's runtime footer off an answer (the desktop's
         * `split_footer`): the footer's index in [body] and what it says.
         * Null when the last paragraph isn't one (prose, a one-word
         * sign-off…). Like the desktop, a `$0.18` part reads as the model
         * name unless a real one came first.
         */
        fun splitFooter(body: String): Pair<Int, Runtime>? {
            val trimmed = body.trimEnd()
            val at = trimmed.lastIndexOf("\n\n")
            if (at < 0) return null
            val start = at + 2
            val line = trimmed.substring(start).trim()
            if (line.isEmpty() || '\n' in line || line.length > 200) return null
            val parts = line.split(" · ").map { it.trim() }
            var model: String? = null
            var ctx: Int? = null
            var cwd: String? = null
            var latency: String? = null
            val digits = { s: String -> s.isNotEmpty() && s.all { it in '0'..'9' } }
            for (part in parts) {
                val pct = part.substringOrNullOnSuffix('%')
                when {
                    pct != null && digits(pct) && pct.length <= 3 -> ctx = pct.toInt()
                    part.startsWith("~") || part.startsWith("/") -> cwd = part
                    part == "<1s" || (part.endsWith("s") &&
                        (digits(part.dropLast(1)) || part.dropLast(1).splitOnceOrNull('m')?.let { (m, s) -> digits(m) && digits(s) } == true)
                        ) -> latency = part
                    model == null && (part.contains(" → ") ||
                        (!part.any { it.isWhitespace() } && part.any { it in '0'..'9' || it == '-' })) -> model = part
                    else -> return null
                }
            }
            // A lone model-ish word could be prose; a % or a path is a footer.
            if (!(parts.size >= 2 || ctx != null || cwd != null)) return null
            return start to Runtime(model = model, contextPct = ctx, cwd = cwd, latency = latency)
        }

        private fun String.substringOrNullOnSuffix(c: Char): String? =
            if (endsWith(c)) dropLast(1) else null

        /** `m` only between digits here; splitOnce semantics. */
        private fun String.splitOnceOrNull(c: Char): Pair<String, String>? {
            val i = indexOf(c)
            return if (i < 0) null else substring(0, i) to substring(i + 1)
        }

        /** "$0.18" / "0.18" / "$1,204.50" → dollars (the desktop's budgets::parse_usd). */
        fun parseUsd(s: String): Double? =
            s.trim().removePrefix("$").replace(",", "").toDoubleOrNull()?.takeIf { it.isFinite() && it >= 0.0 }

        /** The runtime the protocol fields report (the desktop's `Protocol::runtime`). */
        fun ofProtocol(p: Protocol): Runtime? {
            val ctx = p.context?.coerceIn(0f, 1f)?.let { Math.round(it * 100.0).toInt() }
            val rt = Runtime(model = p.model, contextPct = ctx, cwd = p.cwd, costUsd = p.costUsd)
            return if (rt == Runtime()) null else rt
        }

        /**
         * Remember each room's newest runtime from [msgs] (the desktop's
         * `take_footers_of`, minus stripping the footer from the body — the
         * phone shows answers whole): the protocol's fields count for any
         * agent; a footer only when the harness sets `runtime_footer`. A
         * message at or after the stored timestamp replaces it. [footerOf]
         * is the harness's runtime_footer per agent id.
         */
        fun noteAll(runtimes: MutableMap<String, Pair<Long, Runtime>>, roomId: String, msgs: List<Msg>, agents: List<String>, footerOf: (String) -> Boolean) {
            for (m in msgs) {
                if (m.sender !in agents) continue
                val protocol = m.agent
                val fromProtocol = if (protocol != null) Runtime.ofProtocol(protocol) else null
                val fromFooter = if (footerOf(m.sender)) splitFooter(m.body)?.second else null
                val rt = fromProtocol ?: fromFooter ?: continue
                val newer = runtimes[roomId].let { it == null || m.ts >= it.first }
                if (newer) runtimes[roomId] = m.ts to rt
            }
        }
    }
}
