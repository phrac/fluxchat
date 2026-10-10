package org.fluxchat.companion.quick

import android.appwidget.AppWidgetManager
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.History
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import org.fluxchat.companion.matrix.Matrix
import org.fluxchat.companion.ui.AGENT_ROOM_ORDER
import org.fluxchat.companion.ui.Avatar
import org.fluxchat.companion.ui.FluxTheme

/** Which room a new widget starts sessions in (also on reconfiguring it). */
class QuickWidgetConfig : ComponentActivity() {
    @OptIn(ExperimentalMaterial3Api::class)
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val widgetId = intent.getIntExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, AppWidgetManager.INVALID_APPWIDGET_ID)
        if (widgetId == AppWidgetManager.INVALID_APPWIDGET_ID) return finish()
        // Backing out cancels adding the widget.
        setResult(RESULT_CANCELED, Intent().putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, widgetId))
        val pick: (String?) -> Unit = { roomId ->
            Quick.setWidgetRoom(this, widgetId, roomId)
            AppWidgetManager.getInstance(this).updateAppWidget(widgetId, QuickWidget.views(this, widgetId))
            setResult(RESULT_OK, Intent().putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, widgetId))
            finish()
        }
        setContent {
            FluxTheme {
                val rooms by Matrix.agentRooms.collectAsState()
                val ordered = rooms.values.sortedWith(AGENT_ROOM_ORDER)
                Scaffold(topBar = { TopAppBar(title = { Text("Widget: start sessions in…") }) }) { pad ->
                    LazyColumn(Modifier.fillMaxSize().padding(pad)) {
                        item {
                            Row(
                                Modifier.fillMaxWidth().clickable { pick(null) }.padding(horizontal = 16.dp, vertical = 12.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Icon(Icons.Outlined.History, null, Modifier.size(44.dp).padding(10.dp))
                                Spacer(Modifier.width(14.dp))
                                Column {
                                    Text("The room I last used", style = MaterialTheme.typography.titleMedium)
                                    Text("Follows wherever you last sent a request", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                                }
                            }
                        }
                        if (ordered.isEmpty()) item {
                            Text("Finding your projects…", color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(16.dp))
                        }
                        items(ordered, key = { it.roomId }) { r ->
                            Row(
                                Modifier.fillMaxWidth().clickable { pick(r.roomId) }.padding(horizontal = 16.dp, vertical = 10.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Avatar(r.roomId, r.name, r.avatar, 44.dp, room = !r.isDm)
                                Spacer(Modifier.width(14.dp))
                                Text(r.name, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            }
                        }
                    }
                }
            }
        }
    }
}
