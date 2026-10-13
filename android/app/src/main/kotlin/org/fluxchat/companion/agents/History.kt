package org.fluxchat.companion.agents

import org.json.JSONArray
import org.json.JSONObject
import java.util.Locale

/**
 * Run history: every run the phone saw live (so durations are real), with
 * its agent, room, title, start / end, steps, model and cost at the end —
 * the desktop's history.rs ported. Stored encrypted ([org.fluxchat.companion.matrix.HistoryStore]),
 * searchable on the Runs screen, with spend per agent over a period.
 */
data class RunRecord(
    /** The view's stable id: the room, or "room::threadRoot". */
    val key: String,
    val room: String,
    val agent: String,
    val title: String,
    /** "working" / "needs you" / "done" / "failed" / "stalled". */
    val state: String,
    /** ms since the epoch. */
    val started: Long,
    val ended: Long? = null,
    val steps: Int = 0,
    val model: String? = null,
    /** Max cost the agent reported for this run (protocol `cost_usd`). */
    val costUsd: Double? = null,
    /** What was asked (when the phone saw it): lone commands prune on load. */
    val request: String? = null,
    /** How the run ended, one line (protocol `result`), once it has. */
    val result: String? = null,
) {
    /** "4s" / "4m" / "1h 20m" — start to end (or to now while going), like the desktop's `duration`. */
    fun duration(now: Long): String {
        val secs = ((ended ?: now) - started).coerceAtLeast(0) / 1000
        return when {
            secs <= 59 -> "${secs}s"
            secs <= 3599 -> "${secs / 60}m"
            else -> "${secs / 3600}h ${secs % 3600 / 60}m"
        }
    }
}

object History {
    /** Runs kept (oldest dropped): the desktop's HISTORY_LIMIT. */
    const val HISTORY_LIMIT = 500
    /** History state of a run that went quiet while working. */
    const val STALLED = "stalled"
    /** A working run silent this long has stalled (agents.rs STALE_RUN_MS). */
    const val STALE_RUN_MS = 2 * 60 * 60 * 1000L

    /**
     * Start of today / this week containing [nowMs], local time — the
     * desktop budgets.rs `Period::start`: days at 00:00, weeks from Monday
     * 00:00 (not a rolling 7-day window, so both clients count the same runs).
     */
    fun periodStart(nowMs: Long, week: Boolean): Long {
        val cal = java.util.Calendar.getInstance()
        cal.timeInMillis = nowMs
        cal.set(java.util.Calendar.HOUR_OF_DAY, 0)
        cal.set(java.util.Calendar.MINUTE, 0)
        cal.set(java.util.Calendar.SECOND, 0)
        cal.set(java.util.Calendar.MILLISECOND, 0)
        if (week) while (cal.get(java.util.Calendar.DAY_OF_WEEK) != java.util.Calendar.MONDAY)
            cal.add(java.util.Calendar.DAY_OF_YEAR, -1)
        return cal.timeInMillis
    }

    /** `ViewKey` as a history key (the desktop's "thread::…"/room key equivalent). */
    fun keyOf(key: ViewKey): String = key.threadRoot?.let { "${key.roomId}::$it" } ?: key.roomId

    fun stateLabel(state: RunState): String = when (state) {
        RunState.Working -> "working"
        RunState.NeedsYou -> "needs you"
        RunState.Done -> "done"
        RunState.Failed -> "failed"
    }

    /** `!usage`, `/status`: a command with nothing after it — not a task. */
    fun isLoneCommand(text: String): Boolean {
        val t = text.trim()
        return (t.startsWith("!") || t.startsWith("/")) && t.length > 1 && !t.any { it.isWhitespace() }
    }

    /**
     * Note a run's latest state: runs only ever seen live are recorded, and a
     * recorded run that ends gets its end, model and cost (the desktop's
     * `record_run`). [requestBody]: the request behind the run (its thread's
     * root, or the room's request), when known — a lone command isn't a task.
     * Returns whether the history changed.
     */
    fun record(
        history: MutableList<RunRecord>,
        view: ViewKey,
        run: RunInfo,
        title: String,
        requestBody: String?,
        now: Long,
        /** The room's newest runtime info (footer or protocol), for the end backfill. */
        runtime: Runtime? = null,
    ): Boolean {
        val key = keyOf(view)
        val live = run.state.live
        val label = stateLabel(run.state)
        val i = history.indexOfFirst { it.key == key && it.started == run.since }
        return if (i < 0) {
            if (!live) return false
            if (requestBody != null && isLoneCommand(requestBody)) return false
            history += RunRecord(
                key, view.roomId, run.agent, title, label, run.since,
                steps = run.steps, costUsd = run.costUsd, request = requestBody,
                result = run.result,
            )
            val extra = history.size - HISTORY_LIMIT
            if (extra > 0) history.subList(0, extra).clear()
            true
        } else {
            val rec = history[i]
            if (rec.state == STALLED) return false // settled stays settled (the desktop drops stalled states)
            val cost = run.costUsd?.let { c -> rec.costUsd?.let { maxOf(it, c) } ?: c } ?: rec.costUsd
            val model = run.model ?: rec.model
            // Ending with neither the run nor its record carrying model / cost
            // (older agents report no protocol fields on the last edit): the
            // room's runtime info backfills them (the desktop's record_run
            // pulling agent_runtime on the end transition).
            val ending = !live && rec.ended == null
            val costFinal = if (ending && cost == null) runtime?.costUsd else cost
            val modelFinal = if (ending) model ?: runtime?.model else model
            val updated = rec.copy(
                state = label,
                steps = maxOf(rec.steps, run.steps),
                costUsd = costFinal,
                model = modelFinal,
                // The final edit's `result` (nothing else overwrites it).
                result = rec.result ?: run.result,
                ended = if (!live && rec.ended == null) now else rec.ended,
                // A thread's title arrives with its first replies; keep the placeholder till then.
                title = if (rec.title == "Thread" && title != "Thread") title else rec.title,
            )
            history[i] = updated
            // Only the fields that can actually move count as a change (the desktop's tuple compare).
            updated.state != rec.state || updated.steps != rec.steps ||
                updated.costUsd != rec.costUsd || updated.model != rec.model ||
                updated.result != rec.result || updated.ended != rec.ended
        }
    }

    /**
     * Close open records that went quiet while "working" (`stalled`): Hermes
     * leaves its 👀 on requests it never finished after a restart. [lastSign]
     * is the run's latest sign of life (the desktop's `last_sign_of_life`,
     * simplified to the room's latest agent message for that view).
     */
    fun settleStalled(history: MutableList<RunRecord>, lastSign: (RunRecord) -> Long, now: Long): Boolean {
        var changed = false
        for (i in history.indices) {
            val rec = history[i]
            if (rec.state != "working" || rec.ended != null) continue
            val last = maxOf(rec.started, lastSign(rec))
            if (now - last <= STALE_RUN_MS) continue
            history[i] = rec.copy(state = STALLED, ended = last)
            changed = true
        }
        return changed
    }

    /**
     * Drop records of lone commands older builds recorded as runs (the
     * desktop's load_run_history retain). The stored request decides when a
     * record has one; older records without it fall back to the title, which
     * is the request for a thread run. Returns whether anything was dropped.
     */
    fun pruneLoneCommands(history: MutableList<RunRecord>): Boolean {
        val before = history.size
        history.removeAll { isLoneCommand(it.request ?: it.title) }
        return history.size != before
    }

    /** History matching `query` (title, room name, agent, state), newest first. */
    fun matching(history: List<RunRecord>, query: String, roomName: (String) -> String): List<RunRecord> {
        val q = query.trim().lowercase(Locale.US)
        val words = q.splitWhitespace()
        return history.asReversed().filter { r ->
            val hay = "${r.title} ${roomName(r.room)} ${r.agent} ${r.state} ${r.result.orEmpty()}".lowercase(Locale.US)
            words.all { it in hay }
        }
    }

    /**
     * What `agent` spent on runs starting at or after [sinceMs]. Protocol
     * `cost_usd` is the session's running total, so each record of a key
     * counts its increase over the previous record of the same key; a drop
     * is a new session and counts whole (the desktop's `budgets::spent`).
     */
    fun spend(history: List<RunRecord>, agent: String, sinceMs: Long): Double {
        val recs = history.filter { it.agent == agent && it.costUsd != null }.sortedBy { it.started }
        val last = HashMap<String, Double>()
        var total = 0.0
        for (r in recs) {
            val cost = r.costUsd!!
            val prev = last.put(r.key, cost)
            val run = if (prev != null && cost >= prev) cost - prev else cost
            if (r.started >= sinceMs) total += run
        }
        return total
    }

    /**
     * What one record's run itself cost: the increase of its session-total
     * `cost_usd` over the previous record of the same key (whole on a drop) —
     * the number the desktop's Runs dialog formats.
     */
    fun runCost(history: List<RunRecord>, record: RunRecord): Double? {
        val cost = record.costUsd ?: return null
        val prev = history.filter { it.key == record.key && it.started < record.started && it.costUsd != null }
            .maxByOrNull { it.started }?.costUsd
        return if (prev != null && cost >= prev) cost - prev else cost
    }

    // --- JSON codec (the desktop stores serde JSON under the same idea) ---

    fun encode(history: List<RunRecord>): ByteArray {
        val arr = JSONArray()
        for (r in history) {
            val o = JSONObject()
                .put("key", r.key).put("room", r.room).put("agent", r.agent).put("title", r.title)
                .put("state", r.state).put("started", r.started)
            if (r.ended != null) o.put("ended", r.ended)
            o.put("steps", r.steps)
            if (r.model != null) o.put("model", r.model)
            if (r.costUsd != null) o.put("cost_usd", r.costUsd)
            if (r.request != null) o.put("request", r.request)
            if (r.result != null) o.put("result", r.result)
            arr.put(o)
        }
        return arr.toString().toByteArray(Charsets.UTF_8)
    }

    fun decode(bytes: ByteArray): MutableList<RunRecord> {
        val out = ArrayList<RunRecord>()
        runCatching {
            val arr = JSONArray(String(bytes, Charsets.UTF_8))
            for (i in 0 until arr.length()) {
                val o = arr.optJSONObject(i) ?: continue
                val key = o.optString("key")
                if (key.isEmpty()) continue
                out += RunRecord(
                    key = key,
                    room = o.optString("room"),
                    agent = o.optString("agent"),
                    title = o.optString("title"),
                    state = o.optString("state"),
                    started = o.optLong("started"),
                    ended = if (o.has("ended")) o.getLong("ended") else null,
                    steps = o.optInt("steps"),
                    model = o.optString("model").ifEmpty { null },
                    costUsd = if (o.has("cost_usd")) o.getDouble("cost_usd") else null,
                    request = o.optString("request").ifEmpty { null },
                    result = o.optString("result").ifEmpty { null },
                )
            }
        }
        return out
    }

    private fun String.splitWhitespace(): List<String> = split(' ', '\n', '\t').filter { it.isNotEmpty() }
}
