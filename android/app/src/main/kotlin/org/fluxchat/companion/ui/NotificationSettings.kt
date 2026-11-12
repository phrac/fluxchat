package org.fluxchat.companion.ui

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.Settings
import android.widget.Toast
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Notifications
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import androidx.core.app.NotificationManagerCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import org.fluxchat.companion.Diag
import org.fluxchat.companion.matrix.Matrix
import org.fluxchat.companion.push.Push

/** Push through UnifiedPush: on/off, which distributor, and getting ntfy. */
@Composable
fun NotificationSettings() {
    val context = LocalContext.current
    val status by Push.status.collectAsState()
    val on = status is Push.Status.On || status is Push.Status.Registering
    val distributors = Push.distributors(context)
    // Push can be on and working with every notification blocked by Android:
    // it looked fine here while nothing ever showed. Looked at again on the
    // way back from Android's settings.
    var allowed by remember { mutableStateOf(NotificationManagerCompat.from(context).areNotificationsEnabled()) }
    LifecycleEventEffect(Lifecycle.Event.ON_RESUME) { allowed = NotificationManagerCompat.from(context).areNotificationsEnabled() }
    val toggle: (Boolean) -> Unit = { want ->
        // On the app's scope: turning push off talks to the server (the
        // pusher goes), which must neither run on this screen's main-thread
        // scope nor stop half-way when you leave Settings.
        if (want) (context as? Activity)?.let { Push.enable(it) } else Matrix.launch { Push.disable(context.applicationContext) }
    }
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        ListItem(
            // The whole row is the switch: one target, read out as one.
            modifier = Modifier.toggleable(value = on, role = Role.Switch, onValueChange = toggle),
            leadingContent = { Icon(Icons.Outlined.Notifications, null) },
            headlineContent = { Text("Push notifications") },
            supportingContent = {
                Text(
                    when (val s = status) {
                        Push.Status.Off -> "Off: you only hear from agents while the app is open"
                        Push.Status.NoDistributor -> "Needs a UnifiedPush app: install ntfy"
                        Push.Status.Registering -> "Connecting…"
                        is Push.Status.On -> "On, through ${appLabel(context, s.distributor)}"
                        is Push.Status.Failed -> s.reason
                    },
                    color = if (status is Push.Status.Failed) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            },
            trailingContent = { Switch(checked = on, onCheckedChange = null) },
        )
        if (!allowed) {
            Text(
                "Android is blocking fluxchat's notifications, so nothing can show, with or without push.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.error,
                modifier = Modifier.padding(horizontal = 16.dp),
            )
            TextButton(modifier = Modifier.padding(horizontal = 8.dp), onClick = {
                val settings = Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, context.packageName)
                open(context, "Android's notification settings", settings)
            }) { Text("Allow notifications") }
        }
        if (distributors.isEmpty()) {
            Text(
                "Push goes through a UnifiedPush distributor, without Google. ntfy is free, and works with ntfy.sh or your own server.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 16.dp),
            )
            TextButton(modifier = Modifier.padding(horizontal = 8.dp), onClick = {
                val market = Intent(Intent.ACTION_VIEW, Uri.parse("market://details?id=io.heckel.ntfy"))
                val web = Intent(Intent.ACTION_VIEW, Uri.parse("https://ntfy.sh/docs/subscribe/phone/"))
                // No app store, then no browser either: that used to be a crash.
                open(context, "ntfy's page", market, web)
            }) { Text("Get ntfy") }
        } else if (distributors.size > 1) {
            for (d in distributors) {
                TextButton(modifier = Modifier.padding(horizontal = 8.dp), onClick = { Push.use(context, d) }) { Text("Use ${appLabel(context, d)}") }
            }
        }
    }
}

/** Start the first of [intents] something on the phone can open; say so when nothing can. */
private fun open(context: Context, what: String, vararg intents: Intent) {
    var last: Exception? = null
    for (intent in intents) {
        try {
            context.startActivity(intent)
            return
        } catch (e: Exception) {
            last = e
        }
    }
    Diag.error("Opening $what", last)
    Toast.makeText(context, "Nothing on this phone can open $what", Toast.LENGTH_LONG).show()
}

// Shown by its package name when the label can't be read: a fallback, not a failure worth a diagnostics line per redraw.
private fun appLabel(context: Context, pkg: String): String = runCatching {
    val pm = context.packageManager
    pm.getApplicationLabel(pm.getApplicationInfo(pkg, 0)).toString()
}.getOrDefault(pkg)
