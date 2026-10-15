package org.fluxchat.companion.push

import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.onEach
import org.fluxchat.companion.Diag
import org.fluxchat.companion.MainActivity
import org.fluxchat.companion.Notifications
import org.fluxchat.companion.R
import org.fluxchat.companion.agents.RunInfo
import org.fluxchat.companion.agents.RunState
import org.fluxchat.companion.agents.Runs
import org.fluxchat.companion.agents.ViewKey
import org.fluxchat.companion.matrix.Matrix
import org.fluxchat.companion.ui.shortName
import org.fluxchat.companion.ui.threadTitle

/**
 * One quiet, ongoing notification while your agents work: who, what step,
 * for how long, a progress bar and Stop; the runs list when there are
 * several. A foreground service ([RunService]) holds it, which also keeps
 * sync going while the app is in the background (Android freezes it
 * otherwise, and the notification would go stale). Gone when no run is live.
 */
object LiveRuns {
    const val NOTIFICATION_ID = 7301

    /** A working run silent this long has stalled (desktop agents.rs `STALE_RUN_MS`). */
    private const val STALE_RUN_MS = 2 * 60 * 60 * 1000L

    /** A live run, as the notification and the run widget show it. */
    internal data class Live(val key: ViewKey, val run: RunInfo, val agent: String, val title: String, val room: String)

    @Volatile private var last: String? = null
    @Volatile private var current: android.app.Notification? = null
    @Volatile private var serviceUp = false

    fun start(context: Context) {
        val app = context.applicationContext
        Matrix.agentRooms.onEach { update(app) }.launchIn(Matrix.scope)
    }

    private fun live(): List<Live> {
        return Matrix.agentRooms.value.values.flatMap { room ->
            val msgs = Matrix.roomTimeline(room.roomId)?.messages?.value.orEmpty()
            room.runs.mapNotNull { (key, run) ->
                if (run == null || !run.state.live) return@mapNotNull null
                val agentId = run.agent.ifEmpty { room.agents.firstOrNull().orEmpty() }
                // Hermes leaves its 👀 on requests it never finished (after a restart): a
                // working run with no sign of life for a while isn't live (the desktop's STALE_RUN_MS).
                val lastSign = msgs.filter { it.sender == agentId && (it.threadRoot == key.threadRoot || it.id == key.threadRoot) }.maxOfOrNull { it.ts } ?: 0L
                if (run.state == RunState.Working && System.currentTimeMillis() - maxOf(run.since, lastSign) > STALE_RUN_MS) return@mapNotNull null
                val agent = shortName(agentId, msgs.lastOrNull { it.sender == agentId }?.senderName)
                val title = key.threadRoot?.let { r -> threadTitle(msgs.firstOrNull { it.id == r }, msgs.filter { it.threadRoot == r }) } ?: room.name
                Live(key, run, agentId.ifEmpty { agent }, title, room.name)
            }
        }.sortedWith(compareBy({ it.run.rank().first }, { it.run.rank().second }))
    }

    /** Rows for the home-screen run widget: the same live() view as the notification. */
    internal fun widgetRows(): List<org.fluxchat.companion.quick.RunWidgetRow> = live().map { l ->
        org.fluxchat.companion.quick.RunWidgetRow(
            key = l.key.toString(),
            roomId = l.key.roomId,
            threadRoot = l.key.threadRoot,
            title = "${l.agent} · ${l.title}",
            sub = Runs.stepLabel(l.run.step) ?: l.run.state.label,
            stop = stopCommand(l),
        )
    }

    @Synchronized
    private fun update(context: Context) {
        // The home-screen run widget follows every change (it draws from the
        // saved snapshot, so this also keeps it current for a redraw later).
        org.fluxchat.companion.quick.RunWidget.refresh(context)
        val runs = live()
        if (runs.isEmpty()) {
            if (last != null) {
                last = null
                current = null
                if (serviceUp) context.stopService(Intent(context, RunService::class.java))
                NotificationManagerCompat.from(context).cancel(NOTIFICATION_ID)
            }
            return
        }
        // Only re-post when what it says changes (the timer runs by itself).
        val sig = runs.joinToString("|") { "${it.key}/${it.run.state}/${it.run.step}/${it.run.steps}/${it.run.progress}" }
        if (sig == last) return
        last = sig
        val n = build(context, runs)
        current = n
        if (!serviceUp) {
            // From the background Android may refuse a foreground service: the
            // notification still shows (and updates while the process lives).
            runCatching { ContextCompat.startForegroundService(context, Intent(context, RunService::class.java)) }
                .onFailure { Diag.note("live runs: no foreground service (${it.javaClass.simpleName}), plain notification") }
        }
        runCatching { NotificationManagerCompat.from(context).notify(NOTIFICATION_ID, n) }
    }

    /** The notification as it stands, for [RunService.startForeground]. */
    internal fun notification(context: Context): android.app.Notification? = current ?: live().takeIf { it.isNotEmpty() }?.let { build(context, it) }

    internal fun serviceRunning(up: Boolean) {
        serviceUp = up
    }

    private fun build(context: Context, runs: List<Live>): android.app.Notification {
        val top = runs.first()
        val r = top.run
        val agentName = shortName(top.agent, null)
        val b = NotificationCompat.Builder(context, Notifications.LIVE)
            .setSmallIcon(R.drawable.ic_notification)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setSilent(true)
            .setCategory(NotificationCompat.CATEGORY_PROGRESS)
            .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
            .setContentIntent(open(context, top.key))
            .setWhen(r.since)
            .setUsesChronometer(true)
            .setShowWhen(true)
        if (runs.size == 1) {
            b.setContentTitle("$agentName · ${r.state.label}")
                .setContentText(Runs.stepLabel(r.step) ?: top.title)
                .setSubText(top.title.takeIf { it != top.room }?.let { "${top.room} › $it" } ?: top.room)
        } else {
            val waiting = runs.count { it.run.state == RunState.NeedsYou }
            b.setContentTitle(if (waiting > 0) "${runs.size} agent runs · $waiting need you" else "${runs.size} agent runs")
                .setContentText("${top.title}: ${Runs.stepLabel(r.step) ?: r.state.label}")
                .setStyle(NotificationCompat.InboxStyle().also { s ->
                    runs.take(6).forEach { l -> s.addLine("${if (l.run.state == RunState.NeedsYou) "⏸" else "▶"} ${l.title} — ${Runs.stepLabel(l.run.step) ?: l.run.state.label}") }
                })
        }
        when {
            r.state == RunState.NeedsYou -> {}
            r.progress != null -> b.setProgress(100, (r.progress * 100).toInt(), false)
            else -> b.setProgress(0, 0, true)
        }
        stopAction(context, top)?.let { b.addAction(0, "Stop", it) }
        return b.build()
    }

    /** Stop, as the harness takes it (in the run's thread, addressed to the agent where it needs that). */
    internal fun stopCommand(l: Live): String? {
        val h = Matrix.agentList.value.harnessOf(l.agent)
        if (h.stop.isEmpty()) return null
        val c = h.command(h.stop)
        return if (h.mentionCommands && !Matrix.isDm(l.key.roomId)) "${l.agent} $c" else c
    }

    private fun stopAction(context: Context, l: Live): PendingIntent? {
        val text = stopCommand(l) ?: return null
        val i = Intent(context, ActionReceiver::class.java)
            .setAction(ActionReceiver.SEND)
            .putExtra(ActionReceiver.ROOM, l.key.roomId)
            .putExtra(ActionReceiver.EVENT, l.key.threadRoot ?: l.key.roomId)
            .putExtra(ActionReceiver.THREAD, l.key.threadRoot)
            .putExtra(ActionReceiver.VALUE, text)
            .putExtra(ActionReceiver.LABEL, "Stop")
        return PendingIntent.getBroadcast(context, ("stop" + l.key).hashCode(), i, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
    }

    private fun open(context: Context, key: ViewKey): PendingIntent {
        val i = Intent(context, MainActivity::class.java)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
            .putExtra(MainActivity.EXTRA_ROOM, key.roomId)
        key.threadRoot?.let { i.putExtra(MainActivity.EXTRA_THREAD, it) }
        return PendingIntent.getActivity(context, ("live" + key).hashCode(), i, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
    }
}

/**
 * Holds the live runs notification as a foreground service, so sync keeps
 * running (and the notification current) while the app is in the background.
 */
class RunService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val n = LiveRuns.notification(this)
        if (n == null) {
            stopSelf()
            return START_NOT_STICKY
        }
        val type = if (Build.VERSION.SDK_INT >= 29) ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC else 0
        runCatching { ServiceCompat.startForeground(this, LiveRuns.NOTIFICATION_ID, n, type) }
            .onSuccess { LiveRuns.serviceRunning(true) }
            .onFailure {
                Diag.error("Starting the live runs service", it)
                stopSelf()
            }
        return START_NOT_STICKY
    }

    /** Android 15 caps data sync services at 6 hours a day. */
    override fun onTimeout(startId: Int, fgsType: Int) {
        Diag.note("live runs: service timed out")
        stopSelf()
    }

    override fun onDestroy() {
        LiveRuns.serviceRunning(false)
        super.onDestroy()
    }
}
