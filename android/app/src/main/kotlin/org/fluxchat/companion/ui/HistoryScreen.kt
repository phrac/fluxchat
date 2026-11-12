package org.fluxchat.companion.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import java.util.Calendar
import java.util.Locale
import org.fluxchat.companion.agents.History
import org.fluxchat.companion.agents.RunRecord
import org.fluxchat.companion.matrix.Matrix
import org.matrix.rustcomponents.sdk.Membership

/**
 * Run history across all your agent rooms: what was asked, how it ended, how
 * long it took, cost and model — plus spend per agent for today and this
 * week. The phone's port of the desktop's Runs dialog (history side).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HistoryScreen(go: (Screen) -> Unit, back: () -> Unit) {
    val history by Matrix.runHistoryView.collectAsState()
    var query by rememberSaveable { mutableStateOf("") }
    // Live durations tick while a run is still going.
    var now by rememberSaveable { mutableStateOf(System.currentTimeMillis()) }
    LaunchedEffect(Unit) { while (true) { now = System.currentTimeMillis(); delay(1_000) } }
    val matches = History.matching(history, query) { Matrix.roomName(it) }
    // Runs outlive their projects here: a row whose project you have left
    // (or that was removed) opened a screen with nothing to show.
    val rooms by Matrix.agentRooms.collectAsState()
    val recorded = remember(history) { history.mapTo(HashSet()) { it.room } }
    val gone by produceState(emptySet<String>(), recorded, rooms.keys) {
        value = withContext(Dispatchers.IO) { goneProjects(recorded, rooms.keys, ::joined) }
    }
    val keyboard = LocalSoftwareKeyboardController.current
    val scroll = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
    Scaffold(
        modifier = Modifier.nestedScroll(scroll.nestedScrollConnection),
        topBar = {
            LargeTopAppBar(
                title = { Text("Runs") },
                navigationIcon = { IconButton(onClick = back) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") } },
                scrollBehavior = scroll,
            )
        },
    ) { pad ->
        // imePadding: edge to edge, the keyboard otherwise lies over the last rows of a search.
        LazyColumn(Modifier.fillMaxSize().imePadding(), contentPadding = PaddingValues(top = pad.calculateTopPadding(), bottom = 24.dp)) {
            item {
                OutlinedTextField(
                    query,
                    { query = it },
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
                    leadingIcon = { Icon(Icons.Outlined.Search, null) },
                    placeholder = { Text("Search runs") },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    // The rows filter as you type: Search puts the keyboard away to show them (it did nothing).
                    keyboardActions = KeyboardActions(onSearch = { keyboard?.hide() }),
                )
            }
            if (query.isBlank()) {
                item { SpendSummary(history, now) }
            }
            items(matches, key = { "${it.key}@${it.started}" }, contentType = { "run" }) { r ->
                val left = r.room in gone
                ListItem(
                    headlineContent = {
                        Text(
                            r.title.ifBlank { "Run" }, maxLines = 2, overflow = TextOverflow.Ellipsis,
                            color = if (left) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface,
                        )
                    },
                    supportingContent = {
                        Column {
                            // day · time, then state · duration · cost · model
                            Text(dayLine(r.started) + " · " + r.state)
                            if (left) Text("Its project is no longer on this phone", style = MaterialTheme.typography.bodySmall)
                            // How it ended, when the agent says (protocol `result`).
                            r.result?.trim()?.takeIf { it.isNotEmpty() }?.let {
                                Text(it, style = MaterialTheme.typography.bodyMedium, maxLines = 2, overflow = TextOverflow.Ellipsis)
                            }
                            Text(
                                listOfNotNull(
                                    Matrix.roomName(r.room).takeIf { it.isNotBlank() && it != r.room },
                                    shortName(r.agent, null),
                                    r.duration(now),
                                    History.runCost(history, r)?.let { "$" + String.format(Locale.US, "%.2f", it) },
                                    r.model,
                                ).joinToString(" · "),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    },
                    modifier = Modifier
                        // Nothing to open where the project is gone: the row is its record.
                        .clickable(enabled = !left) {
                            val root = r.key.removePrefix(r.room + "::").takeIf { it != r.key }
                            go(if (root != null) Screen.Thread(r.room, root) else Screen.Room(r.room))
                        }
                        .animateItem(),
                )
            }
            if (matches.isEmpty()) {
                item {
                    Text(
                        if (query.isBlank()) "No runs recorded yet on this phone. Runs get recorded while the app sees them work."
                        else "Nothing matches \"${query.trim()}\".",
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(16.dp),
                    )
                }
            }
        }
    }
}

/**
 * The projects among [recorded] that can't be opened any more: not one of
 * your agents' projects now, and not one you are still in ([joined]) either.
 */
internal fun goneProjects(recorded: Set<String>, live: Set<String>, joined: (String) -> Boolean): Set<String> =
    recorded.filterTo(HashSet()) { it !in live && !joined(it) }

/** Whether you are (still) in the project. A failed look counts as "yes": a row is only closed on what we know. */
private fun joined(roomId: String): Boolean = try {
    Matrix.room(roomId)?.let { it.membership() == Membership.JOINED } ?: false
} catch (e: Exception) {
    org.fluxchat.companion.Diag.error("Looking up the project of a recorded run", e)
    true
}

/** Today and this week, per agent: what they spent. */
@Composable
private fun SpendSummary(history: List<RunRecord>, now: Long) {
    // Today / this week's windows from History.periodStart — the desktop's
    // budgets Period::start (weeks from Monday 00:00 local), so the numbers
    // match the desktop's spend for the same runs.
    val today = History.periodStart(now, week = false)
    val week = History.periodStart(now, week = true)
    val agents = history.map { it.agent }.distinct().sorted()
    if (agents.isEmpty()) return
    Column(Modifier.padding(horizontal = 16.dp)) {
        Text("Spend", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
        for (a in agents) {
            val t = History.spend(history, a, today)
            val w = History.spend(history, a, week)
            Row(Modifier.fillMaxWidth().padding(vertical = 4.dp), horizontalArrangement = Arrangement.SpaceBetween) {
                Text(shortName(a, null), style = MaterialTheme.typography.bodyMedium)
                Text(
                    "today ${usd(t)} · week ${usd(w)}",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        HorizontalDivider(Modifier.padding(vertical = 8.dp))
    }
}

private fun usd(c: Double): String = "$" + String.format(Locale.US, "%.2f", c)

private fun dayLine(ms: Long): String {
    val cal = Calendar.getInstance().apply { timeInMillis = ms }
    val today = Calendar.getInstance()
    val stamp = String.format(Locale.US, "%02d:%02d", cal.get(Calendar.HOUR_OF_DAY), cal.get(Calendar.MINUTE))
    val sameDay = cal.get(Calendar.YEAR) == today.get(Calendar.YEAR) && cal.get(Calendar.DAY_OF_YEAR) == today.get(Calendar.DAY_OF_YEAR)
    return if (sameDay) "today $stamp"
    else java.text.SimpleDateFormat("EEE d MMM", Locale.US).format(cal.time) + " $stamp"
}
