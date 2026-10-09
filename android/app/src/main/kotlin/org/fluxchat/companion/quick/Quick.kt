package org.fluxchat.companion.quick

import android.annotation.SuppressLint
import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.Build
import android.service.quicksettings.TileService
import android.widget.RemoteViews
import androidx.core.content.pm.ShortcutInfoCompat
import androidx.core.content.pm.ShortcutManagerCompat
import androidx.core.graphics.drawable.IconCompat
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.onEach
import org.fluxchat.companion.Diag
import org.fluxchat.companion.R
import org.fluxchat.companion.matrix.Matrix
import org.fluxchat.companion.ui.AGENT_ROOM_ORDER
import org.fluxchat.companion.ui.shortName

/**
 * Starting a session from outside the app: the quick prompt sheet
 * ([QuickPromptActivity]) opened by the home-screen widget, the Quick
 * Settings tile and a launcher shortcut per agent room. The room it sends
 * to defaults to the one you last asked something in.
 */
object Quick {
    private const val PREFS = "quick"
    private const val KEY_ROOM = "room"
    /** Launcher shortcuts: Android shows four or five on a long-press. */
    private const val MAX_SHORTCUTS = 4
    const val EXTRA_ROOM = "room"
    const val EXTRA_VOICE = "voice"

    /** The room a quick prompt goes to by default: the last one you sent a request in. */
    fun room(context: Context): String? = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString(KEY_ROOM, null)

    fun remember(context: Context, roomId: String) {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        if (prefs.getString(KEY_ROOM, null) == roomId) return
        prefs.edit().putString(KEY_ROOM, roomId).apply()
        QuickWidget.refresh(context)
    }

    /** The room a widget sends to: its own, or null for the room you last used. */
    fun widgetRoom(context: Context, widgetId: Int): String? =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString("widget:$widgetId", null)?.ifEmpty { null }

    fun setWidgetRoom(context: Context, widgetId: Int, roomId: String?) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putString("widget:$widgetId", roomId.orEmpty()).apply()
    }

    fun forgetWidget(context: Context, widgetId: Int) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().remove("widget:$widgetId").apply()
    }

    /**
     * Room and agent names as last seen, for the widget: it's drawn when the
     * app may not have synced yet (or isn't running).
     */
    fun label(context: Context, roomId: String): Pair<String, String?>? {
        val p = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val name = p.getString("name:$roomId", null) ?: return null
        return name to p.getString("agent:$roomId", null)
    }

    private fun saveLabels(context: Context, labels: List<Triple<String, String, String?>>) {
        val e = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit()
        for ((id, name, agent) in labels) {
            e.putString("name:$id", name)
            if (agent != null) e.putString("agent:$id", agent) else e.remove("agent:$id")
        }
        e.apply()
    }

    /** The quick prompt, for [roomId] (else the usual room), listening at once when [voice]. */
    fun intent(context: Context, roomId: String? = null, voice: Boolean = false): Intent =
        Intent(context, QuickPromptActivity::class.java)
            .setAction(if (voice) "org.fluxchat.companion.QUICK_VOICE" else "org.fluxchat.companion.QUICK")
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK)
            .apply { roomId?.let { putExtra(EXTRA_ROOM, it) } }
            .putExtra(EXTRA_VOICE, voice)

    /** Keep the launcher shortcuts (and the widget's room name) in step with the agent rooms. */
    @OptIn(kotlinx.coroutines.FlowPreview::class)
    fun start(context: Context) {
        val app = context.applicationContext
        Matrix.agentRooms
            .map { rooms ->
                rooms.values.sortedWith(AGENT_ROOM_ORDER).map { r ->
                    val msgs = Matrix.roomTimeline(r.roomId)?.messages?.value.orEmpty()
                    Triple(r.roomId, r.name, r.agents.firstOrNull()?.let { a -> shortName(a, msgs.lastOrNull { it.sender == a }?.senderName) })
                }
            }
            .distinctUntilChanged()
            .debounce(2_000)
            .onEach { all ->
                saveLabels(app, all)
                val top = all.take(MAX_SHORTCUTS).map { it.first to it.second }
                runCatching {
                    val shortcuts = top.mapIndexed { i, (id, name) ->
                        ShortcutInfoCompat.Builder(app, "session:$id")
                            .setShortLabel(name.take(24))
                            .setLongLabel("New session in $name")
                            .setIcon(IconCompat.createWithResource(app, R.mipmap.ic_launcher))
                            .setIntent(intent(app, id))
                            .setRank(i)
                            .build()
                    }
                    ShortcutManagerCompat.setDynamicShortcuts(app, shortcuts)
                }.onFailure { Diag.error("Updating launcher shortcuts", it) }
                QuickWidget.refresh(app)
            }
            .launchIn(Matrix.scope)
    }
}

/**
 * The home-screen widget: a text field ("Ask fbhermes…", its room under it)
 * and a mic. Android widgets can't take typing, so a tap opens the quick
 * prompt with the keyboard up; the mic opens it listening. Each widget has
 * its room (picked when it's added, [QuickWidgetConfig]), or follows the one
 * you last used.
 */
class QuickWidget : AppWidgetProvider() {
    override fun onUpdate(context: Context, manager: AppWidgetManager, ids: IntArray) {
        ids.forEach { manager.updateAppWidget(it, views(context, it)) }
    }

    override fun onDeleted(context: Context, ids: IntArray) {
        ids.forEach { Quick.forgetWidget(context, it) }
    }

    companion object {
        fun refresh(context: Context) {
            val manager = AppWidgetManager.getInstance(context)
            manager.getAppWidgetIds(ComponentName(context, QuickWidget::class.java)).forEach { manager.updateAppWidget(it, views(context, it)) }
        }

        fun views(context: Context, widgetId: Int): RemoteViews {
            val own = Quick.widgetRoom(context, widgetId)
            val room = own ?: Quick.room(context)
            val label = room?.let { Quick.label(context, it) }
            val flags = PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            return RemoteViews(context.packageName, R.layout.widget_quick).apply {
                setTextViewText(R.id.widget_title, "Ask ${label?.second ?: "your agent"}…")
                setTextViewText(
                    R.id.widget_room,
                    when {
                        label == null -> context.getString(R.string.new_session)
                        own == null -> "New session · ${label.first} (last used)"
                        else -> "New session · ${label.first}"
                    },
                )
                // Request codes per widget: each one opens its own room.
                setOnClickPendingIntent(R.id.widget_root, PendingIntent.getActivity(context, widgetId * 2, Quick.intent(context, room), flags))
                setOnClickPendingIntent(R.id.widget_mic, PendingIntent.getActivity(context, widgetId * 2 + 1, Quick.intent(context, room, voice = true), flags))
            }
        }
    }
}

/** The Quick Settings tile: a new session from anywhere. */
class QuickTile : TileService() {
    @SuppressLint("StartActivityAndCollapseDeprecated")
    override fun onClick() {
        val intent = Quick.intent(this)
        if (Build.VERSION.SDK_INT >= 34) {
            startActivityAndCollapse(PendingIntent.getActivity(this, 3, intent, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE))
        } else {
            @Suppress("DEPRECATION")
            startActivityAndCollapse(intent)
        }
    }
}
