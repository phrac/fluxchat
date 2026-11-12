package org.fluxchat.companion.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.consumeWindowInsets
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
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import org.fluxchat.companion.agents.SearchResult
import org.fluxchat.companion.agents.SessionSearch
import org.fluxchat.companion.matrix.Matrix
import org.fluxchat.companion.matrix.Session

/**
 * Find a session by what it was about, across every project. Two engines
 * merged: word-AND matching over each session's title and latest reply (the
 * server thread list, so it works offline against loaded pages), and the
 * store's full-text search, which reaches inside every reply — those results
 * show the matching line and open the session scrolled to it.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SearchScreen(go: (Screen) -> Unit, back: () -> Unit, @Suppress("UNUSED_PARAMETER") swap: (Screen) -> Unit) {
    val rooms by Matrix.agentRooms.collectAsState()
    var query by rememberSaveable { mutableStateOf("") }
    // The box takes the keyboard when you arrive — once: coming back from a
    // result, it is the results you came back for, not the keyboard over them.
    var arrived by rememberSaveable { mutableStateOf(false) }
    val box = remember { FocusRequester() }
    val focus = LocalFocusManager.current
    val keyboard = LocalSoftwareKeyboardController.current
    var all by rememberSaveable { mutableStateOf(false) }
    var loading by rememberSaveable { mutableStateOf(false) }
    var batches by remember { mutableIntStateOf(0) }
    val lists = rooms.keys.mapNotNull { roomId ->
        Matrix.sessionListNow(roomId)?.let { roomId to it }
    }
    val now by rememberNow()
    val search = Matrix.serverSearch
    // Server full-text hits stream in as diffs while the query stands.
    val hits by (search?.hits ?: remember { kotlinx.coroutines.flow.MutableStateFlow(emptyList<org.fluxchat.companion.matrix.ReplyHit>()) }).collectAsState()
    // Open every project's list once, then pull older pages while results
    // still want them; `batches` re-gathers the snapshot as pages arrive.
    LaunchedEffect(rooms.keys.size) {
        loading = true
        // A list that throws while opening must not take the app down with
        // it (nothing above an effect catches), nor leave "Loading…" up.
        try {
            Matrix.sessionsEach(rooms.keys.toList())
            all = true
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            org.fluxchat.companion.Diag.error("Opening the session lists to search", e)
        } finally {
            loading = false
        }
    }
    LaunchedEffect(all, loading, batches) {
        while (all) {
            val pending = rooms.keys.mapNotNull { r -> Matrix.sessionListNow(r)?.takeIf { !it.endReachedNow } }
            if (pending.isEmpty()) break
            delay(1_500)
            pending.forEach { it.more() }
            batches++
        }
    }
    // Query the full-text index as the box settles (not per keystroke).
    LaunchedEffect(query, search) {
        val s = search ?: return@LaunchedEffect
        delay(400)
        val trimmed = query.trim()
        if (trimmed.isNotBlank()) s.search(trimmed)
    }
    val snapshot: List<Pair<String, Session>> = lists.flatMap { (roomId, l) ->
        l.itemsNow.map { roomId to it }
    }
    val trimmed = query.trim()
    // The hits answer the query last asked: while the box is ahead of it
    // (typing), the old query's hits are not this one's results.
    val asked by (search?.query ?: remember { kotlinx.coroutines.flow.MutableStateFlow<String?>(null) }).collectAsState()
    val current = if (asked == trimmed) hits else emptyList()
    val matches = remember(trimmed, snapshot, current, batches) {
        SessionSearch.merge(trimmed, snapshot, current) { Matrix.roomName(it) }
    }
    val scroll = TopAppBarDefaults.enterAlwaysScrollBehavior()
    Scaffold(
        modifier = Modifier.nestedScroll(scroll.nestedScrollConnection),
        topBar = {
            TopAppBar(
                title = { Text(if (trimmed.isEmpty()) "Search sessions" else "${matches.size} match${if (matches.size == 1) "" else "es"}") },
                navigationIcon = { IconButton(onClick = back) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") } },
                scrollBehavior = scroll,
            )
        },
    ) { pad ->
        // imePadding: edge to edge, the keyboard otherwise lies over the last results.
        Column(Modifier.padding(pad).consumeWindowInsets(pad).fillMaxSize().imePadding()) {
            OutlinedTextField(
                value = query,
                onValueChange = { query = it },
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp).focusRequester(box),
                placeholder = { Text("What was the session about?") },
                leadingIcon = { Icon(Icons.Outlined.Search, null) },
                singleLine = true,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                // Results are already there as you type: Search puts the keyboard away to show them.
                keyboardActions = KeyboardActions(onSearch = { keyboard?.hide(); focus.clearFocus() }),
            )
            // Here, beside the field: its focus requester exists once this
            // part of the screen has been composed, not before.
            LaunchedEffect(Unit) {
                if (!arrived) {
                    arrived = true
                    withFrameNanos { }
                    try {
                        box.requestFocus()
                    } catch (e: IllegalStateException) {
                        org.fluxchat.companion.Diag.error("Focusing the search box", e)
                    }
                }
            }
            if (trimmed.isEmpty()) {
                Text(
                    if (loading) "Loading your sessions…" else "Type what you remember from the session — any word in any reply matches.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(16.dp),
                )
            }
            val state = androidx.compose.foundation.lazy.rememberLazyListState()
            val nearBottom by remember {
                derivedStateOf {
                    val info = state.layoutInfo
                    (info.visibleItemsInfo.lastOrNull()?.index ?: 0) >= info.totalItemsCount - 4
                }
            }
            LaunchedEffect(nearBottom, trimmed, hits.size, search?.loadingNow) {
                if (nearBottom && trimmed.isNotBlank()) search?.more()
            }
            LazyColumn(state = state, modifier = Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
                items(matches, key = { "${it.roomId}|${it.root}" }) { m ->
                    ListItem(
                        headlineContent = {
                            when (m) {
                                is SearchResult.SessionHit ->
                                    Text(m.title.ifBlank { "Session" }, maxLines = 2, overflow = TextOverflow.Ellipsis)
                                is SearchResult.Reply ->
                                    Text(Matrix.roomName(m.roomId), maxLines = 1, overflow = TextOverflow.Ellipsis)
                            }
                        },
                        supportingContent = {
                            Column {
                                when (m) {
                                    is SearchResult.SessionHit ->
                                        Text("Session", maxLines = 1, color = MaterialTheme.colorScheme.onSurfaceVariant)
                                    is SearchResult.Reply -> {
                                        Text("“${m.snippet}”", maxLines = 2, overflow = TextOverflow.Ellipsis,
                                            color = MaterialTheme.colorScheme.onSurfaceVariant)
                                    }
                                }
                                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                    if (m is SearchResult.Reply) {
                                        Text(Matrix.roomName(m.roomId), style = MaterialTheme.typography.labelMedium)
                                        m.senderName?.let { Text(it, style = MaterialTheme.typography.labelMedium,
                                            color = MaterialTheme.colorScheme.onSurfaceVariant) }
                                    }
                                    Text(elapsed(m.lastTs, now), style = MaterialTheme.typography.labelMedium,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant)
                                }
                            }
                        },
                        // go, not swap: Back from the session returns to these
                        // results (swapped out, it went past them to Projects).
                        modifier = Modifier.clickable {
                            keyboard?.hide()
                            go(Screen.Thread(m.roomId, m.root, eventOf(m)))
                        },
                    )
                    HorizontalDivider()
                }
            }
        }
    }
}

/** The reply event to scroll to (null: the session's own title is the match). */
private fun eventOf(m: SearchResult): String? = (m as? SearchResult.Reply)?.eventId
