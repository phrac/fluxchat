package org.fluxchat.companion.matrix

import android.app.ApplicationExitInfo
import android.content.Context
import android.content.SharedPreferences
import android.os.Build

/**
 * Self-heal for a poisoned local store. A native crash inside
 * libmatrix_sdk_ffi (matrix-rust-sdk #7204/#7248 family) leaves the event
 * cache desynced ON DISK: every later member lookup re-reads the poison and
 * the next project creation crashes again — even on an app build that
 * passed acceptance before the bad data existed (t_aa662dd8). So at the
 * next start, notice the native crash and rebuild the store in place:
 * Client.clearCaches drops the event cache and the state store, and the
 * client is rebuilt against the clean one. The crypto store and the session
 * survive — no sign-out, no recovery key — and the next sync refills the
 * cache from the server.
 *
 * Call order: [bootStarted] runs at app init and decides once; a later
 * [healRequested] tells the session restore; Matrix.rebuildStore calls
 * [healDone] when it has rebuilt.
 */
object StoreHeal {
    private const val PREFS = "store-heal"
    private const val KEY_ACK = "acked_crash_ms"
    private const val KEY_HEALS = "heals"

    /** Healing twice in a poisoned streak means the crash isn't (only) the store. */
    const val MAX_HEALS = 2

    /** Debuggerd reports a native SIGSEGV/SIGBUS as REASON_CRASH_NATIVE. */
    private val NATIVE_CRASH = ApplicationExitInfo.REASON_CRASH_NATIVE

    @Volatile private var pending = false

    private fun prefs(context: Context): SharedPreferences =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    /** The pure policy: heal only after an unacknowledged native crash, and never more than [maxHeals] in a row. */
    fun decide(unackedNativeCrash: Boolean, heals: Int, maxHeals: Int = MAX_HEALS): Boolean =
        unackedNativeCrash && heals < maxHeals

    /**
     * First thing at app start: look at how the last process died. A
     * native crash newer than the last acknowledged one arms the heal;
     * anything else (a clean death, the user swiping away, an ANR) proves
     * the store was working, resets the streak and moves the watermark on.
     */
    fun bootStarted(context: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) return // ApplicationExitInfo is API 30
        val p = prefs(context)
        val ack = p.getLong(KEY_ACK, 0L)
        if (crashedSince(context, ack)) {
            pending = decide(true, p.getInt(KEY_HEALS, 0))
        } else {
            pending = false
            p.edit().putInt(KEY_HEALS, 0).putLong(KEY_ACK, System.currentTimeMillis()).apply()
        }
    }

    /** This start arms a store rebuild (decided at [bootStarted], once). */
    fun healRequested(): Boolean = pending

    /** The store was rebuilt for this crash: count it and stop re-reacting to it. */
    fun healDone(context: Context) {
        pending = false
        val p = prefs(context)
        p.edit()
            .putInt(KEY_HEALS, p.getInt(KEY_HEALS, 0) + 1)
            .putLong(KEY_ACK, System.currentTimeMillis())
            .apply()
    }

    /** True when the process died from a native crash after [since] (0 = all history). */
    private fun crashedSince(context: Context, since: Long): Boolean {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) return false
        val am = context.getSystemService(android.app.ActivityManager::class.java) ?: return false
        val exits = runCatching {
            am.getHistoricalProcessExitReasons(context.packageName, 0, 16)
        }.getOrDefault(emptyList())
        return exits.any { it.timestamp > since && it.reason == NATIVE_CRASH }
    }
}
