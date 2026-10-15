package org.fluxchat.companion.quick

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.view.View
import android.widget.RemoteViews
import org.fluxchat.companion.MainActivity
import org.fluxchat.companion.R
import org.fluxchat.companion.push.ActionReceiver
import org.fluxchat.companion.push.LiveRuns
import org.json.JSONArray
import org.json.JSONObject

/** One live run, as the widget saves and draws it. */
data class RunWidgetRow(
    val key: String,
    val roomId: String,
    val threadRoot: String?,
    val title: String,
    val sub: String,
    /** The harness's stop command, already mention-addressed; null = no Stop for it. */
    val stop: String?,
)

/**
 * The last widget snapshot, in prefs: a widget draws whenever the launcher
 * asks, including when our process died — so it reads what LiveRuns last
 * saw rather than the live matrix (org.json ships with Android).
 */
object RunWidgetData {
    private const val PREFS = "runwidget"
    private const val KEY_ROWS = "rows"
    /** Beyond this the card scrolls off its cells; the notification lists more. */
    const val MAX_ROWS = 4

    fun save(context: Context, rows: List<RunWidgetRow>) {
        val a = JSONArray()
        for (r in rows.take(MAX_ROWS)) {
            a.put(
                JSONObject()
                    .put("k", r.key).put("r", r.roomId)
                    .put("t", r.threadRoot ?: JSONObject.NULL)
                    .put("a", r.title).put("s", r.sub)
                    .put("x", r.stop ?: JSONObject.NULL),
            )
        }
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putString(KEY_ROWS, a.toString()).apply()
    }

    fun load(context: Context): List<RunWidgetRow> {
        val raw = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString(KEY_ROWS, null) ?: return emptyList()
        return runCatching {
            val a = JSONArray(raw)
            (0 until a.length()).map { i ->
                val o = a.getJSONObject(i)
                RunWidgetRow(
                    key = o.getString("k"),
                    roomId = o.getString("r"),
                    threadRoot = o.optString("t").ifEmpty { null },
                    title = o.getString("a"),
                    sub = o.getString("s"),
                    stop = o.optString("x").ifEmpty { null },
                )
            }
        }.getOrDefault(emptyList())
    }

    fun clear(context: Context) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().remove(KEY_ROWS).apply()
    }
}

/**
 * The home-screen run widget: one row per live agent run — title, current
 * step, Stop — so the home screen is a status board. Tap a row to open the
 * session; when nothing runs it invites a new session instead. It follows
 * the same live() view as the ongoing notification (LiveRuns pushes every
 * refresh; a period tick redraws when the process has died).
 */
class RunWidget : AppWidgetProvider() {
    override fun onUpdate(context: Context, manager: AppWidgetManager, ids: IntArray) {
        ids.forEach { manager.updateAppWidget(it, views(context)) }
    }

    companion object {
        /** Snapshot the live runs and redraw every placed widget. */
        fun refresh(context: Context) {
            val rows = LiveRuns.widgetRows()
            RunWidgetData.save(context, rows)
            push(context)
        }

        fun push(context: Context) {
            val manager = AppWidgetManager.getInstance(context)
            val ids = manager.getAppWidgetIds(ComponentName(context, RunWidget::class.java))
            if (ids.isEmpty()) return
            val views = views(context)
            ids.forEach { manager.updateAppWidget(it, views) }
        }

        fun views(context: Context): RemoteViews {
            val rows = RunWidgetData.load(context)
            val flags = PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            return RemoteViews(context.packageName, R.layout.widget_runs).apply {
                val empty = rows.isEmpty()
                setViewVisibility(R.id.widget_runs_empty, if (empty) View.VISIBLE else View.GONE)
                setViewVisibility(R.id.widget_runs_list, if (empty) View.GONE else View.VISIBLE)
                setOnClickPendingIntent(
                    R.id.widget_runs_root,
                    PendingIntent.getActivity(context, 7000, Quick.intent(context), flags),
                )
                removeAllViews(R.id.widget_runs_list)
                for (r in rows) {
                    val row = RemoteViews(context.packageName, R.layout.widget_run_row).apply {
                        setTextViewText(R.id.widget_run_title, r.title)
                        setTextViewText(R.id.widget_run_sub, r.sub)
                        setOnClickPendingIntent(
                            R.id.widget_run_row,
                            PendingIntent.getActivity(context, r.key.hashCode(), openIntent(context, r), flags),
                        )
                        if (r.stop == null) {
                            setViewVisibility(R.id.widget_run_stop, View.GONE)
                        } else {
                            setTextViewText(R.id.widget_run_stop, context.getString(R.string.widget_run_stop))
                            setOnClickPendingIntent(
                                R.id.widget_run_stop,
                                PendingIntent.getBroadcast(context, ("stop" + r.key).hashCode(), stopIntent(context, r), flags),
                            )
                        }
                    }
                    addView(R.id.widget_runs_list, row)
                }
            }
        }

        private fun openIntent(context: Context, r: RunWidgetRow): Intent {
            val i = Intent(context, MainActivity::class.java)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
                .putExtra(MainActivity.EXTRA_ROOM, r.roomId)
            r.threadRoot?.let { i.putExtra(MainActivity.EXTRA_THREAD, it) }
            return i
        }

        /** Stop, exactly as the notification's Stop sends it (ActionReceiver.SEND). */
        private fun stopIntent(context: Context, r: RunWidgetRow): Intent =
            Intent(context, ActionReceiver::class.java)
                .setAction(ActionReceiver.SEND)
                .putExtra(ActionReceiver.ROOM, r.roomId)
                .putExtra(ActionReceiver.EVENT, r.threadRoot ?: r.roomId)
                .putExtra(ActionReceiver.THREAD, r.threadRoot)
                .putExtra(ActionReceiver.VALUE, r.stop.orEmpty())
                .putExtra(ActionReceiver.LABEL, "Stop")
    }
}
