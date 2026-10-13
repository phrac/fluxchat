package org.fluxchat.companion.agents

/**
 * The agent's runtime info (the desktop's agents.rs `Runtime`): from the
 * protocol's fields (`Protocol.runtime()` there) or Hermes's footer
 * (`/footer on`) — the last paragraph of a final answer, `model · 42% ·
 * ~/dir` (any of those, plus latency like `22s`, or `alias → served`). The
 * newest one per room backfills a run record's model / cost when the run
 * ends ([History.record]'s runtime argument — the desktop's `record_run`
 * pulling `agent_runtime` on the end transition).
 */
data class Runtime(
    val model: String? = null,
    /** Context window used, whole percent (protocol `context` 0.0–1.0). */
    val contextPct: Int? = null,
    val cwd: String? = null,
    val latency: String? = null,
    /** Spend so far (protocol `cost_usd`); the footer itself carries no money. */
    val costUsd: Double? = null,
) {
    companion object {
        /** `claude-opus-4 · ctx 42% · $0.18 · ~/proj` — what's known (the desktop's `summary`). */
        fun Runtime.summary(): String = listOfNotNull(
            model,
            contextPct?.let { "ctx $it%" },
            costUsd?.let { "\$" + String.format(java.util.Locale.US, "%.2f", it) },
            cwd,
        ).joinToString(" \u00b7 ")

        /**
         * Split Hermes's runtime footer off an answer (the desktop's
         * `split_footer`): the footer's index in [body] and what it says.
         * Null when the last paragraph isn't one (prose, a one-word sign-off…).
         */
        fun splitFooter(body: String): Pair<Int, Runtime>? {
            val trimmed = body.trimEnd()
            val at = trimmed.lastIndexOf("\n\n")
            if (at < 0) return null
            val start = at + 2
            val line = trimmed.substring(start).trim()
            if (line.isEmpty() || '\n' in line || line.length > 200) return null
            val parts = line.split(" \u00b7 ").map { it.trim() }
            var model: String? = null
            var ctx: Int? = null
            var cwd: String? = null
            var latency: String? = null
            for (part in parts) {
                val digits = { s: String -> s.isNotEmpty() && s.all { it in '0'..'9' } }
                when {
                    part.endsWith("%") && digits(part.dropLast(1)) && part.length - 1 <= 3 ->
                        ctx = part.dropLast(1).toIntOrNull()
                    part.startsWith("~") || part.startsWith("/") -> cwd = part
                    part == "<1s" || (part.endsWith("s") && digits(part.dropLast(1))) ||
                        part.endsWith("s") && run {
                            val m = part.dropLast(1).split('m', limit = 2)
                            m.size == 2 && digits(m[0]) && m[1].all { it in '0'..'9' }
                        } -> latency = part
                    model == null && (part.contains(" \u2192 ") ||
                        (!part.any { it.isWhitespace() } && part.any { it in '0'..'9' || it == '-' })) -> model = part
                    else -> return null
                }
            }
            // A lone model-ish word could be prose; a % or a path is a footer.
            if (!(parts.size >= 2 || ctx != null || cwd != null)) return null
            return start to Runtime(model = model, contextPct = ctx, cwd = cwd, latency = latency)
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
    }
}
