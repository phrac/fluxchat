package org.fluxchat.companion.ui

import android.app.Activity
import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
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
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.fluxchat.companion.push.Push

/** Push through UnifiedPush: on/off, which distributor, and getting ntfy. */
@Composable
fun NotificationSettings() {
    val context = LocalContext.current
    val status by Push.status.collectAsState()
    val scope = rememberCoroutineScope()
    val on = status is Push.Status.On || status is Push.Status.Registering
    val distributors = Push.distributors(context)
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        ListItem(
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
            trailingContent = {
                Switch(checked = on, onCheckedChange = { want ->
                    if (want) (context as? Activity)?.let { Push.enable(it) } else scope.launch { Push.unregister(context) }
                })
            },
        )
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
                runCatching { context.startActivity(market) }.onFailure { context.startActivity(web) }
            }) { Text("Get ntfy") }
        } else if (distributors.size > 1) {
            for (d in distributors) {
                TextButton(modifier = Modifier.padding(horizontal = 8.dp), onClick = { Push.use(context, d) }) { Text("Use ${appLabel(context, d)}") }
            }
        }
    }
}

private fun appLabel(context: android.content.Context, pkg: String): String = runCatching {
    val pm = context.packageManager
    pm.getApplicationLabel(pm.getApplicationInfo(pkg, 0)).toString()
}.getOrDefault(pkg)
