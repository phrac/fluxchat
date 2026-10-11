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

    /**
     * How long a process must live before its non-native death counts as
     * proof the store loaded fine: a crash-looping process is force-stopped
     * or ANRs seconds after the last crash, which proves nothing.
     */
    const val HEALTHY_RUN_MS = 60_000L

    /** Debuggerd reports a native SIGSEGV as REASON_CRASH_NATIVE; SIGBUS arrives as REASON_SIGNALED (t_aa662dd8 field data). */
    private val NATIVE_CRASH = setOf(ApplicationExitInfo.REASON_CRASH_NATIVE, ApplicationExitInfo.REASON_SIGNALED)

    @Volatile private var pending = false

    private fun prefs(context: Context): SharedPreferences =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    /** The pure policy: heal only after an unacknowledged native crash, and never more than [maxHeals] in a row. */
    fun decide(unackedNativeCrash: Boolean, heals: Int, maxHeals: Int = MAX_HEALS): Boolean =
        unackedNativeCrash && heals < maxHeals

    /**
     * First thing at app start: read the exit history. A native crash newer
     * than the last acknowledged one arms the heal — a force-stop or swipe
     * after the crash does NOT excuse the poison, so benign exits do not
     * mask it. A process that lived a healthy minute and then died of
     * something else proves the store was fine: reset the streak and move
     * the watermark past everything seen.
     */
    fun bootStarted(context: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) return // ApplicationExitInfo is API 30
        val p = prefs(context)
        val ack = p.getLong(KEY_ACK, 0L)
        val exits = exits(context)
        if (exits.any { it.reason in NATIVE_CRASH && it.timestamp > ack }) {
            pending = decide(true, p.getInt(KEY_HEALS, 0))
        } else {
            pending = false
            val provedHealthy = exits.any {
                it.reason !in NATIVE_CRASH && it.timestamp > ack && it.duration > HEALTHY_RUN_MS
            }
            if (provedHealthy) {
                val newest = (exits.maxOfOrNull { it.timestamp } ?: 0L).coerceAtLeast(ack)
                p.edit().putInt(KEY_HEALS, 0).putLong(KEY_ACK, newest).apply()
            }
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

    private fun exits(context: Context): List<ApplicationExitInfo> {
        val am = context.getSystemService(android.app.ActivityManager::class.java) ?: return emptyList()
        return runCatching { am.getHistoricalProcessExitReasons(context.packageName, 0, 64) }
            .getOrDefault(emptyList())
    }
}
