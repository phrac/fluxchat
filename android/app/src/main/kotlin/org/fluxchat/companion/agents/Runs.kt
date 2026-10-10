package org.fluxchat.companion.agents

/**
 * Reading runs out of a room's messages: a port of the desktop's agents.rs
 * (`request_state`, `prompt_choices`, `pending_question`, `progress_steps`,
 * `protocol_runs`, `agent_updates`). The phone keeps one timeline per agent
 * room with thread replies in it, so a thread's view is its root plus the
 * replies that name it.
 */
object Runs {
    /** Prompts older than this aren't live any more. */
    const val PROMPT_MAX_AGE_MS = 60L * 60 * 1000
    private const val STEP_MAX_CHARS = 300

    /** The state an agent's reactions give a request (one of your messages). */
    fun requestState(h: Harness, msg: Msg, agent: String): RunState? {
        fun byAgent(emoji: String) = emoji.isNotEmpty() &&
            msg.reactions.any { sameEmoji(it.emoji, emoji) && agent in it.users }
        // Done / failed win: the working one is taken back a few seconds later.
        return when {
            byAgent(h.failedReaction) -> RunState.Failed
            byAgent(h.doneReaction) -> RunState.Done
            byAgent(h.workingReaction) -> RunState.Working
            else -> null
        }
    }

    /**
     * Whether the agent posted after `msgs[i]` (step lines aside: Hermes's
     * heartbeat runs on a timer): a prompt of its is over then.
     */
    fun movedOn(msgs: List<Msg>, i: Int, agent: String): Boolean =
        msgs.drop(i + 1).any { it.sender == agent && !it.isLocal && it.isMessage && progressSteps(it.body) == null }

    fun markedOver(msg: Msg, h: Harness): Boolean {
        val body = msg.body.trimStart()
        return h.promptOverMarkers.any { it.isNotEmpty() && body.startsWith(it) }
    }

    /**
     * A question asked as text (Hermes's clarify: "❓ …", then "  1. choice")
     * still waiting in this view: its numbered choices (empty: answer in your
     * own words). Null = not one, or answered / superseded.
     */
    fun pendingQuestion(msgs: List<Msg>, i: Int, agent: String, h: Harness, now: Long): List<Pair<String, String>>? {
        val msg = msgs.getOrNull(i) ?: return null
        val prefix = h.questionPrefix
        if (prefix.isEmpty() || msg.sender != agent || !msg.body.trimStart().startsWith(prefix) || now - msg.ts > PROMPT_MAX_AGE_MS) {
            return null
        }
        if (msgs.drop(i + 1).any { !it.isLocal && it.isMessage && (it.sender != agent || progressSteps(it.body) == null) }) {
            return null
        }
        return msg.body.lines().mapNotNull { l ->
            val t = l.trim()
            val dot = t.indexOf(". ")
            if (dot <= 0) return@mapNotNull null
            val n = t.substring(0, dot)
            val rest = t.substring(dot + 2).trim()
            if (n.all { it.isDigit() } && rest.isNotEmpty()) n to rest else null
        }
    }

    /** A reaction prompt's choices: the emoji its sender reacted with, labelled. */
    fun promptChoices(msg: Msg, now: Long, labels: List<Choice>): List<Pair<String, String>> {
        if (now - msg.ts > PROMPT_MAX_AGE_MS) return emptyList()
        // In the harness's order (Approve first), not the order reactions came in.
        val rank = { e: String -> labels.indexOfFirst { sameEmoji(it.emoji, e) }.let { if (it < 0) labels.size else it } }
        return msg.reactions.filter { msg.sender in it.users }
            .sortedBy { rank(it.emoji) }
            .map { it.emoji to choiceLabel(it.emoji, msg.body, labels) }
    }

    private fun bare(e: String) = e.trimEnd('️')
    private fun sameEmoji(a: String, b: String) = bare(a) == bare(b)

    fun choiceLabel(emoji: String, body: String, labels: List<Choice>): String {
        labels.firstOrNull { sameEmoji(it.emoji, emoji) }?.let { return it.label }
        when (bare(emoji)) {
            "✅" -> return "Approve once"
            "🌀" -> return "For this session"
            "♾" -> return "Always"
            "❌", "❎" -> return "Deny"
        }
        return body.lines().firstNotNullOfOrNull { l ->
            val t = l.trim()
            if (t.startsWith(emoji)) t.removePrefix(emoji).trim().replace("`", "").takeIf { it.isNotEmpty() } else null
        } ?: emoji
    }

    /**
     * An approval you've answered: "✅ Approve once" from a person's reaction
     * with one of the harness's approval emoji on an agent message asking for
     * approval (null while it's open, or for other messages — a ✅ on an
     * ordinary answer is just a reaction).
     */
    fun answeredApproval(msg: Msg, h: Harness, agents: Collection<String>): String? {
        if (h.approvals.isEmpty() || !msg.body.contains("approv", ignoreCase = true)) return null
        val r = msg.reactions.firstOrNull { r -> h.approvals.any { sameEmoji(it.emoji, r.emoji) } && r.users.any { it !in agents } } ?: return null
        return "${r.emoji} ${choiceLabel(r.emoji, msg.body, h.approvals)}"
    }

    /**
     * A run's step as a status line shows it: a heartbeat ("⏳ Working — 3 min
     * — iteration 17/150, execute_code") cut to what's new in it, since the
     * line already says Working and for how long.
     */
    fun stepLabel(step: String?): String? {
        val s = step?.replace("`", "") ?: return null
        return if (isHeartbeat(s)) s.substringAfterLast(" — ").takeIf { it.isNotBlank() && it != s } ?: s else s
    }

    /** Hermes's timer heartbeat ("⏳ Working — 3 min — iteration 15/150, terminal"): a status line, not tool work. */
    fun isHeartbeat(body: String): Boolean = body.trim().let { b -> b.startsWith("⏳") && '\n' !in b }

    /** One line of a progress message: a tool emoji, a space, what it's doing. */
    fun isStep(line: String): Boolean {
        val sp = line.indexOf(' ')
        if (sp <= 0) return false
        val head = line.substring(0, sp)
        return line.substring(sp + 1).isNotBlank() && line.codePointCount(0, line.length) <= STEP_MAX_CHARS && isEmoji(head)
    }

    /**
     * The steps of a progress message: every line a step. A terminal command
     * is a fenced block instead (`💻 terminal` then the command in a fence, or
     * just the fence for back-to-back commands): one step, `💻 {command}`,
     * replacing the header before it. Same as the desktop's `progress_steps`.
     */
    fun progressSteps(body: String): List<String>? {
        val lines = body.lines().map { it.trim() }.filter { it.isNotEmpty() }
        val steps = ArrayList<String>()
        var emoji = "💻"
        var i = 0
        while (i < lines.size) {
            val line = lines[i]
            if (line.startsWith("```")) {
                // A command belongs to a step log only after a step (its header or an earlier tool).
                if (steps.isEmpty()) return null
                val end = (i + 1 until lines.size).firstOrNull { lines[it].startsWith("```") } ?: return null
                val cmd = lines.subList(i + 1, end).joinToString(" ")
                if (cmd.isEmpty()) return null
                // `💻 terminal` right before the block: the block says what it ran.
                val last = steps.last()
                if (last == lines[i - 1] && ' ' !in last.substringAfter(' ').trim()) {
                    steps.removeAt(steps.size - 1)
                    emoji = last.substringBefore(' ')
                }
                steps += "$emoji $cmd"
                i = end + 1
            } else if (isStep(line)) {
                steps += line
                i++
            } else {
                return null
            }
        }
        return steps.ifEmpty { null }
    }

    /** Whether `s` is one emoji (incl. ZWJ sequences, skin tones, keycaps, flags). */
    fun isEmoji(s: String): Boolean {
        if (s.isEmpty()) return false
        var sawPictograph = false
        var i = 0
        while (i < s.length) {
            val cp = s.codePointAt(i)
            i += Character.charCount(cp)
            when {
                cp == 0xFE0F || cp == 0x200D || cp == 0x20E3 || cp in 0x1F3FB..0x1F3FF || cp in 0xE0020..0xE007F -> {}
                cp in 0x1F1E6..0x1F1FF -> sawPictograph = true // regional indicators (flags)
                cp >= 0x1F000 -> sawPictograph = true
                cp in 0x2190..0x2BFF || cp in 0x3030..0x303D || cp in 0x3297..0x3299 || cp == 0x00A9 || cp == 0x00AE ->
                    sawPictograph = true
                cp in '0'.code..'9'.code || cp == '#'.code || cp == '*'.code -> if (!s.contains('⃣')) return false
                else -> return false
            }
        }
        return sawPictograph || s.contains('⃣')
    }

    /**
     * A view's messages: the room's own (no thread replies), or a thread's
     * root and replies.
     */
    fun viewMessages(all: List<Msg>, key: ViewKey): List<Msg> = when (val root = key.threadRoot) {
        null -> all.filter { it.threadRoot == null }
        else -> all.filter { it.id == root || it.threadRoot == root }
    }

    /** Thread roots with replies loaded among `all`. */
    private fun loadedThreads(all: List<Msg>): Set<String> = all.mapNotNullTo(HashSet()) { it.threadRoot }

    /** The choices of a prompt `msgs[i]` that's still open in this view (protocol, reactions). */
    fun openPrompt(msgs: List<Msg>, i: Int, h: Harness, now: Long): List<Pair<String, String>> {
        val msg = msgs[i]
        msg.agent?.takeIf { it.choices.isNotEmpty() }?.let { p ->
            return if (p.runState() == RunState.NeedsYou) p.choices.map { it.key to it.label } else emptyList()
        }
        if (movedOn(msgs, i, msg.sender) || markedOver(msg, h)) return emptyList()
        return promptChoices(msg, now, h.approvals)
    }

    /** Whether `agent` answers in threads here: as its last finished request went, else its habit. */
    fun repliesInThreads(roomMsgs: List<Msg>, agent: String, h: Harness, isDm: Boolean): Boolean =
        roomMsgs.asReversed().firstOrNull { it.isOwn && it.threadRoot == null && requestState(h, it, agent) == RunState.Done }
            ?.hasThread ?: (h.replyInThread && !isDm)

    data class Result(val runs: Map<ViewKey, RunInfo?>, val prompts: List<String>)

    /** What `agent`'s signals say about the runs in a room (all its loaded messages). */
    fun agentUpdates(roomId: String, all: List<Msg>, agent: String, h: Harness, isDm: Boolean, now: Long): Result {
        val updates = LinkedHashMap<ViewKey, RunInfo?>()
        val prompts = ArrayList<String>()
        val roomMsgs = viewMessages(all, ViewKey(roomId))
        val loaded = loadedThreads(all)

        fun viewPrompts(msgs: List<Msg>): List<Msg> = msgs.indices.filter { i ->
            val m = msgs[i]
            m.sender == agent && (openPrompt(msgs, i, h, now).isNotEmpty() || pendingQuestion(msgs, i, agent, h, now) != null)
        }.map { msgs[it] }

        fun withStep(msgs: List<Msg>, r: RunInfo): RunInfo {
            if (!h.progressSteps) return r
            val steps = msgs.asReversed().asSequence().filter { it.sender == agent && it.ts >= r.since }
                .firstNotNullOfOrNull { progressSteps(it.body) } ?: return r
            return r.copy(step = steps.last(), steps = steps.size)
        }

        fun settle(latest: RunInfo?, needsYou: Boolean): RunInfo? = when {
            latest != null && needsYou -> latest.copy(state = RunState.NeedsYou)
            latest == null && needsYou -> RunInfo(RunState.NeedsYou, now)
            else -> latest
        }

        // Threads with replies loaded: like the desktop's thread views.
        for (root in loaded) {
            val key = ViewKey(roomId, root)
            val msgs = viewMessages(all, key)
            val open = viewPrompts(msgs)
            prompts += open.map { it.id }
            val latest = msgs.asReversed().firstNotNullOfOrNull { m ->
                if (m.isOwn) requestState(h, m, agent)?.let { RunInfo(it, m.ts) } else null
            }
            updates[key] = settle(latest, open.isNotEmpty())?.let { withStep(msgs, it) }
        }

        // The room: threaded requests whose replies aren't loaded go by the
        // root's reactions; the latest unthreaded one is the room's run.
        val expectThread = repliesInThreads(roomMsgs, agent, h, isDm)
        var latestUnthreaded: RunInfo? = null
        for (m in roomMsgs) {
            if (!m.isOwn) continue
            val state = requestState(h, m, agent) ?: continue
            val threaded = m.hasThread || m.id in loaded || (expectThread && state == RunState.Working)
            val tk = ViewKey(roomId, m.id)
            if (m.id !in loaded && threaded) updates[tk] = RunInfo(state, m.ts)
            if (!threaded) latestUnthreaded = RunInfo(state, m.ts)
        }
        val open = viewPrompts(roomMsgs)
        prompts += open.map { it.id }
        updates[ViewKey(roomId)] = settle(latestUnthreaded, open.isNotEmpty())?.let { withStep(roomMsgs, it) }

        // The protocol wins: several runs on one view → the newest, or one waiting on you.
        val byKey = HashMap<ViewKey, RunInfo>()
        for ((k, run) in protocolRuns(roomId, all, agent)) {
            val old = byKey[k]
            if (old == null || compareValuesBy(run, old, { it.state == RunState.NeedsYou }, { it.since }) > 0) byKey[k] = run
        }
        updates.putAll(byKey)
        return Result(updates.mapValues { (_, r) -> r?.copy(agent = agent) }, prompts)
    }

    /** Runs the agent describes with the protocol, keyed by the view they're in. */
    fun protocolRuns(roomId: String, all: List<Msg>, agent: String): List<Pair<ViewKey, RunInfo>> {
        data class Latest(val msg: Msg, val p: Protocol)
        val latest = HashMap<String, Latest>()
        val openPrompt = HashSet<String>()
        val cost = HashMap<String, Double>()
        for (m in all) {
            if (m.sender != agent) continue
            val p = m.agent ?: continue
            if (p.runState() == null) continue
            val runId = p.run ?: ""
            p.costUsd?.takeIf { it.isFinite() && it >= 0 }?.let { c -> cost[runId] = maxOf(cost[runId] ?: c, c) }
            if (p.choices.isNotEmpty()) {
                if (p.runState() == RunState.NeedsYou) openPrompt += runId
                if (runId in latest) continue
            }
            val old = latest[runId]
            if (old == null || old.p.choices.isNotEmpty() || m.ts >= old.msg.ts) latest[runId] = Latest(m, p)
        }
        return latest.mapNotNull { (runId, l) ->
            val request = all.firstOrNull { it.id == runId }
            val target = when {
                l.msg.threadRoot != null -> ViewKey(roomId, l.msg.threadRoot)
                request?.threadRoot != null -> ViewKey(roomId, request.threadRoot)
                request != null && request.hasThread -> ViewKey(roomId, runId)
                else -> ViewKey(roomId)
            }
            val state = if (runId in openPrompt) RunState.NeedsYou else l.p.runState() ?: return@mapNotNull null
            target to RunInfo(
                state = state,
                since = request?.ts ?: l.msg.ts,
                step = l.p.step,
                steps = l.p.steps ?: 0,
                progress = l.p.progress?.coerceIn(0f, 1f),
                agent = agent,
                costUsd = cost[runId],
                model = l.p.model,
                context = l.p.context,
                result = l.p.result,
            )
        }
    }

    /** Every agent's view of a room's runs, merged (the livelier wins per view). */
    fun roomRuns(roomId: String, all: List<Msg>, agents: List<String>, harnessOf: (String) -> Harness, isDm: Boolean, now: Long): Result {
        val merged = LinkedHashMap<ViewKey, RunInfo?>()
        val prompts = ArrayList<String>()
        for (agent in agents) {
            val r = agentUpdates(roomId, all, agent, harnessOf(agent), isDm, now)
            prompts += r.prompts
            for ((k, run) in r.runs) {
                val old = merged[k]
                if (!merged.containsKey(k) || (old == null && run != null) ||
                    (old != null && run != null && compareValuesBy(run, old, { it.rank().first }, { it.rank().second }) < 0)
                ) {
                    merged[k] = run
                }
            }
        }
        return Result(merged, prompts)
    }

    /** Busy-mode prefix for a message to a working agent ("!steer "), or null. */
    fun busyPrefix(h: Harness, mode: BusyMode): String? {
        val name = when (mode) {
            BusyMode.Send -> return null
            BusyMode.Steer -> h.steer
            BusyMode.Queue -> h.queue
        }
        return if (name.isEmpty()) null else h.command(name) + " "
    }

    /**
     * What to send for typed text to an agent: `/cmd` as the harness takes it
     * (`!cmd`), and while it works, the busy mode's prefix. `addressedTo`:
     * the agent ID, for harnesses that want it in front outside DMs.
     */
    fun outgoing(h: Harness, text: String, working: Boolean, mode: BusyMode, addressedTo: String?): String {
        fun addressed(s: String) = if (h.mentionCommands && addressedTo != null) "$addressedTo $s" else s
        if (text.startsWith("//")) return text.substring(1)
        if (text.startsWith("/")) return addressed(h.commandPrefix + text.substring(1))
        if (text.startsWith("!") || !working) return text
        return busyPrefix(h, mode)?.let { addressed(it + text) } ?: text
    }
}

enum class BusyMode(val label: String) {
    Send("Send"), Steer("Steer"), Queue("Queue");

    companion object {
        fun available(h: Harness) = entries.filter { it == Send || Runs.busyPrefix(h, it) != null }
    }
}
