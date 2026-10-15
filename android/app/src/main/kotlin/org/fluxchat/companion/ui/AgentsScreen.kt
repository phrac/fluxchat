package org.fluxchat.companion.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.outlined.Logout
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.BugReport
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.History
import androidx.compose.material.icons.outlined.Info
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material.icons.outlined.SmartToy
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.fluxchat.companion.AppLock
import org.fluxchat.companion.agents.Harnesses
import org.fluxchat.companion.matrix.AgentEntry
import org.fluxchat.companion.matrix.Matrix

private val USER_ID = Regex("^@[^:\\s]+:\\S+$")

/** Your agents: the account's list, shared with desktop fluxchat. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AgentsScreen(back: () -> Unit) {
    val list by Matrix.agentList.collectAsState()
    val error by Matrix.error.collectAsState()
    val scope = rememberCoroutineScope()
    var adding by remember { mutableStateOf(false) }
    var editing by remember { mutableStateOf<AgentEntry?>(null) }
    val save: (List<AgentEntry>) -> Unit = { agents -> scope.launch { Matrix.saveAgentList(list.copy(agents = agents)) } }
    val scroll = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
    Scaffold(
        modifier = Modifier.nestedScroll(scroll.nestedScrollConnection),
        topBar = {
            LargeTopAppBar(
                title = { Text("Your agents") },
                navigationIcon = { IconButton(onClick = back) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") } },
                scrollBehavior = scroll,
            )
        },
        floatingActionButton = {
            ExtendedFloatingActionButton(
                onClick = { adding = true },
                icon = { Icon(Icons.Outlined.Add, null) },
                text = { Text("Add agent") },
            )
        },
    ) { pad ->
        LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(top = pad.calculateTopPadding(), bottom = 96.dp)) {
            item {
                Text(
                    "Shared with desktop fluxchat through your account: agents you mark there show up here, and the other way round.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                )
            }
            items(list.agents, key = { it.userId }) { a ->
                ListItem(
                    leadingContent = { Avatar(a.userId, shortName(a.userId, null), null, 40.dp) },
                    headlineContent = { Text(shortName(a.userId, null)) },
                    supportingContent = { Text("${a.userId} · ${Harnesses.get(a.harness).name}") },
                    modifier = Modifier.clickable { editing = a }.animateItem(),
                )
            }
            error?.let { e -> item { Text(e, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(16.dp)) } }
        }
    }

    if (adding) {
        var id by remember { mutableStateOf("") }
        val valid = USER_ID.matches(id) && list.agents.none { it.userId == id }
        AlertDialog(
            onDismissRequest = { adding = false },
            icon = { Icon(Icons.Outlined.SmartToy, null) },
            title = { Text("Add an agent") },
            text = {
                OutlinedTextField(
                    id, { id = it.trim() }, singleLine = true, modifier = Modifier.fillMaxWidth(),
                    label = { Text("Agent ID") }, placeholder = { Text("@hermes:example.org") },
                )
            },
            confirmButton = { TextButton(onClick = { save(list.agents + AgentEntry(id)); adding = false }, enabled = valid) { Text("Add") } },
            dismissButton = { TextButton(onClick = { adding = false }) { Text("Cancel") } },
        )
    }

    editing?.let { a ->
        ModalBottomSheet(onDismissRequest = { editing = null }) {
            Column(Modifier.navigationBarsPadding().padding(bottom = 16.dp)) {
                ListItem(
                    leadingContent = { Avatar(a.userId, shortName(a.userId, null), null, 48.dp) },
                    headlineContent = { Text(shortName(a.userId, null), style = MaterialTheme.typography.titleMedium) },
                    supportingContent = { Text(a.userId) },
                    colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow),
                )
                Text(
                    "Runs on",
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                )
                val current = Harnesses.get(a.harness).id
                for (h in Harnesses.list()) {
                    Row(
                        Modifier.fillMaxWidth().clickable {
                            save(list.agents.map { if (it.userId == a.userId) it.copy(harness = h.id) else it })
                            editing = null
                        }.padding(horizontal = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(selected = h.id == current, onClick = null, modifier = Modifier.padding(12.dp))
                        Text(h.name, style = MaterialTheme.typography.bodyLarge)
                    }
                }
                HorizontalDivider(Modifier.padding(vertical = 8.dp))
                ListItem(
                    leadingContent = { Icon(Icons.Outlined.Delete, null, tint = MaterialTheme.colorScheme.error) },
                    headlineContent = { Text("Not an agent", color = MaterialTheme.colorScheme.error) },
                    supportingContent = { Text("Remove from your agents (on every device)") },
                    modifier = Modifier.clickable {
                        save(list.agents.filter { it.userId != a.userId })
                        editing = null
                    },
                    colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow),
                )
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(go: (Screen) -> Unit, back: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var confirm by remember { mutableStateOf(false) }
    val scroll = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
    val list by Matrix.agentList.collectAsState()
    val lockOn by AppLock.enabled.collectAsState()
    Scaffold(
        modifier = Modifier.nestedScroll(scroll.nestedScrollConnection),
        topBar = {
            LargeTopAppBar(
                title = { Text("Settings") },
                navigationIcon = { IconButton(onClick = back) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") } },
                scrollBehavior = scroll,
            )
        },
    ) { pad ->
        Column(Modifier.fillMaxSize().padding(top = pad.calculateTopPadding()).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Section("Account")
            ListItem(
                leadingContent = { Avatar(Matrix.userId, shortName(Matrix.userId, null), null, 40.dp) },
                headlineContent = { Text(shortName(Matrix.userId, null)) },
                supportingContent = { Text(Matrix.userId) },
            )
            ListItem(
                leadingContent = { Icon(Icons.Outlined.SmartToy, null) },
                headlineContent = { Text("Your agents") },
                supportingContent = { Text(if (list.agents.isEmpty()) "None yet" else list.agents.joinToString { shortName(it.userId, null) }) },
                modifier = Modifier.clickable { go(Screen.Agents) },
            )
            Section("Notifications")
            NotificationSettings()
            Section("Runs")
            ListItem(
                leadingContent = { Icon(Icons.Outlined.History, null) },
                headlineContent = { Text("Run history") },
                supportingContent = { Text("What your agents worked on, how it ended, and what it cost") },
                modifier = Modifier.clickable { go(Screen.History) },
            )
            Section("Security")
            ListItem(
                leadingContent = { Icon(Icons.Outlined.Lock, null) },
                headlineContent = { Text("Require unlock") },
                supportingContent = {
                    Text(
                        when {
                            lockOn -> "Fingerprint, face, or screen lock when you open the app; hidden from the app switcher"
                            !AppLock.canEnable(context) -> "Needs a fingerprint, face, or screen lock set up on this phone"
                            else -> "Ask for biometrics or the screen lock when opening fluxchat"
                        },
                    )
                },
                trailingContent = {
                    Switch(
                        checked = lockOn,
                        enabled = lockOn || AppLock.canEnable(context),
                        onCheckedChange = { AppLock.setEnabled(context, it) },
                    )
                },
            )
            Section("About")
            ListItem(
                leadingContent = { Icon(Icons.Outlined.BugReport, null) },
                headlineContent = { Text("Diagnostics") },
                supportingContent = { Text("Sync, rooms, push and recent events, to copy into a bug report") },
                modifier = Modifier.clickable { go(Screen.Diagnostics) },
            )
            ListItem(
                leadingContent = { Icon(Icons.Outlined.Info, null) },
                headlineContent = { Text("fluxchat for Android") },
                supportingContent = { Text("The agent companion to desktop fluxchat") },
            )
            HorizontalDivider(Modifier.padding(vertical = 8.dp))
            ListItem(
                leadingContent = { Icon(Icons.AutoMirrored.Outlined.Logout, null, tint = MaterialTheme.colorScheme.error) },
                headlineContent = { Text("Sign out", color = MaterialTheme.colorScheme.error) },
                modifier = Modifier.clickable { confirm = true },
            )
        }
    }
    if (confirm) {
        AlertDialog(
            onDismissRequest = { confirm = false },
            title = { Text("Sign out of this phone?") },
            text = { Text("Your past conversations need the recovery key again after you sign back in.") },
            confirmButton = { TextButton(onClick = { confirm = false; scope.launch { Matrix.logout() } }) { Text("Sign out") } },
            dismissButton = { TextButton(onClick = { confirm = false }) { Text("Cancel") } },
        )
    }
}

@Composable
private fun Section(title: String) {
    Text(
        title,
        style = MaterialTheme.typography.labelLarge,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 4.dp),
    )
}
