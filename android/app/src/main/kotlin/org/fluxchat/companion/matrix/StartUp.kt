package org.fluxchat.companion.matrix

import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.ensureActive

/**
 * Whether the Projects screen should still say "loading" rather than "no
 * agents yet" / "no projects yet": both empty states carry a button, and at
 * a cold start they flashed by one after the other while the agent list and
 * the rooms were simply not read yet.
 */
object Loading {
    /**
     * Past this the screen shows what there is. Offline with nothing in the
     * store the answers never come, and a spinner for ever says even less
     * than the empty state does.
     */
    const val GIVE_UP_MS = 20_000L

    /**
     * [agentListRead]: the agent list has been read once (the store had it,
     * or the server said what it is — including "not set").
     * [roomListReady]: the room list has delivered what it holds.
     * [looked]: a look through the rooms has finished since both were true.
     */
    fun still(agentListRead: Boolean, roomListReady: Boolean, looked: Boolean, gaveUp: Boolean): Boolean =
        !gaveUp && !(agentListRead && roomListReady && looked)
}

/**
 * Account data the server said isn't set, remembered for a while. With no
 * pins saved (most accounts) and nothing in the store, every room-list change
 * asked the server again and got the same 404; the agent list likewise, plus
 * once a minute.
 *
 * Safe to remember because the store is still read first every time: when
 * another device sets it, sync delivers it there. The memory only stands in
 * for the request that covers "sync hasn't sent it yet".
 */
internal class NotSetCache(private val ttlMs: Long = TTL_MS, private val now: () -> Long = System::currentTimeMillis) {
    private val until = HashMap<String, Long>()
    private var writes = 0

    /** Whether the server recently said [type] isn't set. */
    @Synchronized fun notSet(type: String): Boolean {
        val t = until[type] ?: return false
        if (now() < t) return true
        until.remove(type)
        return false
    }

    /** Before asking the server: a mark that tells [answered] whether we wrote in between. */
    @Synchronized fun asking(): Int = writes

    /**
     * The server answered ([set]: it has the event). A "not set" that was
     * asked before our own write landed is already out of date — remembered,
     * it would read as "no agents" until the echo arrived.
     */
    @Synchronized fun answered(type: String, set: Boolean, asked: Int) {
        if (set) until.remove(type) else if (asked == writes) until[type] = now() + ttlMs
    }

    /** We just set [type] ourselves. */
    @Synchronized fun wrote(type: String) {
        writes++
        until.remove(type)
    }

    /** Ask again next time (the user asked for fresh data, or signed out). */
    @Synchronized fun forget(type: String? = null) {
        if (type == null) until.clear() else until.remove(type)
    }

    companion object {
        const val TTL_MS = 10 * 60_000L
    }
}

/**
 * Whether an uncaught throw in a background coroutine should still take the
 * process down (Matrix's exception handler). [oneOff]: the coroutine was a
 * task that may simply end — a send, a refresh. Anything else that reaches
 * the handler is a coroutine nobody will start again, possibly a collector:
 * ending it quietly would leave the app looking alive and doing nothing, so
 * it stays fatal until it is moved under [keepRunning]. An [Error] always is.
 */
internal fun endsTheProcess(e: Throwable, oneOff: Boolean): Boolean = e is Error || !oneOff

/** How long a failed background loop waits before it is started again: 1 s, doubling to a minute. */
internal fun restartDelayMs(failures: Int): Long = (1_000L shl (failures - 1).coerceIn(0, 6)).coerceAtMost(60_000L)

/** A loop that ran this long before failing wasn't failing in a loop: its next restart is quick again. */
internal const val RESTART_HEALTHY_MS = 60_000L

/**
 * Run [block] until it returns, starting it again whenever it throws. For
 * the loops the app can't do without (working out runs from the timelines,
 * watching the project list): with an exception handler on the scope a throw
 * no longer kills the process — but a loop that just *stops* is worse than a
 * crash, because nothing says so: runs stay "Working", prompts never notify.
 * So each failure is reported and the loop comes back, with a growing pause
 * (a failure that repeats shows up in Diagnostics once a minute, not in a
 * tight spin). Cancellation is not a failure and ends it.
 */
internal suspend fun keepRunning(
    what: String,
    report: (String, Throwable) -> Unit,
    pause: suspend (Long) -> Unit = { delay(it) },
    now: () -> Long = System::currentTimeMillis,
    block: suspend () -> Unit,
) {
    var failures = 0
    while (true) {
        val began = now()
        try {
            block()
            return
        } catch (e: Exception) {
            // Our own cancellation ends the loop. A CancellationException that
            // came from inside while we are still wanted is a failure like
            // any other: let through, it would end the loop without a word.
            currentCoroutineContext().ensureActive()
            failures = if (now() - began >= RESTART_HEALTHY_MS) 1 else failures + 1
            val wait = restartDelayMs(failures)
            report("$what stopped (starting it again in ${wait / 1000} s)", e)
            pause(wait)
        }
    }
}

/**
 * Whether an invite failed only because the user is already there. A new
 * project invites its agents twice (in the create request, then one by one
 * for servers that ignore that field), and an agent that accepted in between
 * answers the second with "already in the room": expected, not an error.
 */
internal fun alreadyInRoom(error: String?): Boolean {
    val m = error?.lowercase() ?: return false
    return "already" in m && ("in the room" in m || "invited" in m || "joined" in m || "member" in m)
}
