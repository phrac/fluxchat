package org.fluxchat.companion

import android.util.Log
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * What the app has been doing, for Settings → Diagnostics: recent events and
 * errors (also logged to logcat), and push counters. Never tokens or keys.
 */
object Diag {
    private const val TAG = "fluxchat"
    private const val KEEP = 150
    private val events = ArrayDeque<String>()
    private val time = SimpleDateFormat("HH:mm:ss", Locale.US)

    @Volatile var pushes = 0
        private set
    @Volatile var lastPush: Long = 0
        private set
    @Volatile var notified = 0
        private set

    fun note(msg: String) {
        Log.i(TAG, msg)
        add("  $msg")
    }

    fun error(msg: String, e: Throwable? = null) {
        Log.w(TAG, msg, e)
        add("! $msg" + (e?.let { ": ${it.javaClass.simpleName}: ${it.message}" } ?: ""))
    }

    fun pushReceived(roomId: String, eventId: String) {
        pushes++
        lastPush = System.currentTimeMillis()
        note("push: $eventId in $roomId")
    }

    fun notificationShown(kind: String, eventId: String) {
        notified++
        note("notified ($kind): $eventId")
    }

    private fun add(line: String) = synchronized(events) {
        events.addLast("${time.format(Date())} $line")
        while (events.size > KEEP) events.removeFirst()
    }

    fun recent(): List<String> = synchronized(events) { events.toList() }
}
