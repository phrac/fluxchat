package org.fluxchat.companion.push

import android.content.Context
import android.os.PowerManager
import org.fluxchat.companion.Diag

/**
 * A short partial wake lock around work a broadcast started and that
 * outlives it. The UnifiedPush connector holds its own only until
 * `onMessage` returns (and its `goAsync` ends there too), which is before
 * the event has even been fetched: on a sleeping phone the CPU could stop
 * between the push arriving and the notification going up.
 *
 * Always with a timeout: a lock that is never released is worse than a late
 * notification.
 */
internal class Awake private constructor(private val lock: PowerManager.WakeLock?) {
    fun release() {
        try {
            if (lock?.isHeld == true) lock.release()
        } catch (e: RuntimeException) {
            // "WakeLock under-locked": it timed out by itself between the check and the release.
            Diag.note("wake lock was already released (${e.message})")
        }
    }

    companion object {
        /** Long enough for a cold start, a fetch and a second look at an event whose keys were late. */
        const val PUSH_MS = 60_000L

        fun hold(context: Context, why: String, maxMs: Long = PUSH_MS): Awake {
            val lock = try {
                context.getSystemService(PowerManager::class.java)
                    ?.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "fluxchat:$why")
                    ?.apply {
                        setReferenceCounted(false)
                        acquire(maxMs)
                    }
            } catch (e: Exception) {
                Diag.error("Holding a wake lock for $why", e)
                null
            }
            return Awake(lock)
        }
    }
}
