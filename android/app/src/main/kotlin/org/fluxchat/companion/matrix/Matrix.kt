@file:OptIn(kotlinx.coroutines.FlowPreview::class)

package org.fluxchat.companion.matrix

import android.content.Context
import org.fluxchat.companion.Diag
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import org.fluxchat.companion.agents.History
import org.fluxchat.companion.agents.Harness
import org.fluxchat.companion.agents.Harnesses
import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.agents.RunInfo
import org.fluxchat.companion.agents.RunRecord
import org.fluxchat.companion.agents.RunState
import org.fluxchat.companion.agents.Runs
import org.fluxchat.companion.agents.ViewKey
import org.fluxchat.companion.ui.threadTitle
import org.json.JSONArray
import org.json.JSONObject
import org.matrix.rustcomponents.sdk.Client
import org.matrix.rustcomponents.sdk.ClientBuilder
import org.matrix.rustcomponents.sdk.ClientSessionDelegate
import org.matrix.rustcomponents.sdk.CreateRoomParameters
import org.matrix.rustcomponents.sdk.DateDividerMode
import org.matrix.rustcomponents.sdk.MembershipState
import org.matrix.rustcomponents.sdk.NotificationClient
import org.matrix.rustcomponents.sdk.NotificationProcessSetup
import org.matrix.rustcomponents.sdk.RecoveryState
import org.matrix.rustcomponents.sdk.Room
import org.matrix.rustcomponents.sdk.RoomListEntriesDynamicFilterKind
import org.matrix.rustcomponents.sdk.RoomListEntriesListener
import org.matrix.rustcomponents.sdk.RoomListEntriesUpdate
import org.matrix.rustcomponents.sdk.RoomPreset
import org.matrix.rustcomponents.sdk.RoomVisibility
import org.matrix.rustcomponents.sdk.Session
import org.matrix.rustcomponents.sdk.SlidingSyncVersionBuilder
import org.matrix.rustcomponents.sdk.SqliteStoreBuilder
import org.matrix.rustcomponents.sdk.SyncService
import org.matrix.rustcomponents.sdk.SyncServiceState
import org.matrix.rustcomponents.sdk.SyncServiceStateObserver
import org.matrix.rustcomponents.sdk.TaskHandle
import org.matrix.rustcomponents.sdk.TimelineConfiguration
import org.matrix.rustcomponents.sdk.TimelineFilter
import org.matrix.rustcomponents.sdk.TimelineFocus
import org.matrix.rustcomponents.sdk.TypingNotificationsListener
import org.matrix.rustcomponents.sdk.VerificationState
import uniffi.matrix_sdk_ui.TimelineReadReceiptTracking
import java.io.File

/** Where sign-in stands. */
sealed interface Auth {
    data object Starting : Auth
    data object SignedOut : Auth
    /** Signed in on a new device: the recovery key unlocks encrypted history. */
    data object NeedsRecovery : Auth
    /**
     * A saved session exists but restoring it just failed (keystore busy,
     * store locked by a half-dead process). The session is NOT dropped — the
     * desktop's rule — so this shows "try again", never the sign-in form.
     */
    data object RestoreFailed : Auth
    data object Ready : Auth
}

/** One of your agents (the account's `org.fluxchat.agents` list). */
data class AgentEntry(val userId: String, val harness: String? = null)

data class AgentList(val agents: List<AgentEntry> = emptyList(), val notAgents: List<String> = emptyList()) {
    fun harnessOf(userId: String): Harness = Harnesses.get(agents.firstOrNull { it.userId == userId }?.harness)

    fun toJson(): String = JSONObject().apply {
        put("agents", JSONArray(agents.map { a -> JSONObject().put("user_id", a.userId).apply { a.harness?.let { put("harness", it) } } }))
        if (notAgents.isNotEmpty()) put("not_agents", JSONArray(notAgents))
    }.toString()

    companion object {
        const val TYPE = "org.fluxchat.agents"

        fun parse(json: String?): AgentList? {
            val o = json?.let { runCatching { JSONObject(it) }.getOrNull() } ?: return null
            val agents = o.optJSONArray("agents")?.let { arr ->
                (0 until arr.length()).mapNotNull { i ->
                    val a = arr.optJSONObject(i) ?: return@mapNotNull null
                    val id = a.optString("user_id").takeIf { it.startsWith("@") } ?: return@mapNotNull null
                    AgentEntry(id, a.optString("harness").ifEmpty { null })
                }
            } ?: emptyList()
            val not = o.optJSONArray("not_agents")?.let { arr -> (0 until arr.length()).map { arr.optString(it) } } ?: emptyList()
            return AgentList(agents, not)
        }
    }
}

/** An agent room as the room list shows it. */
data class AgentRoom(
    val roomId: String,
    val name: String,
    val isDm: Boolean,
    val agents: List<String>,
    /** Runs per view (the room, its threads). */
    val runs: Map<ViewKey, RunInfo?> = emptyMap(),
    val prompts: List<String> = emptyList(),
    val last: Msg? = null,
    val unread: Int = 0,
    val avatar: String? = null,
) {
    /** The run the room's row shows: one waiting on you, else the newest live one, else the newest. */
    val headline: Pair<ViewKey, RunInfo>?
        get() {
            val all = runs.mapNotNull { (k, r) -> r?.let { k to it } }
            return all.filter { it.second.state == org.fluxchat.companion.agents.RunState.NeedsYou }.maxByOrNull { it.second.since }
                ?: all.filter { it.second.state.live }.maxByOrNull { it.second.since }
                ?: all.maxByOrNull { it.second.since }
        }
}

/**
 * The Matrix side of the app: the client, sync, the agent list and live
 * timelines of the rooms your agents are in.
 */
object Matrix {
    private const val TAG = "fluxchat"
    private const val AGENT_LIST_POLL_MS = 60_000L
    /** Own-message ↔ run-start match tolerance (echo vs server timestamps). */
    private const val REQUEST_MATCH_SLACK_MS = 2_000L

    val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private lateinit var appContext: Context
    private lateinit var vault: Vault
    private val startLock = Mutex()

    @Volatile var client: Client? = null
        private set
    private var syncService: SyncService? = null
    private var notifications: NotificationClient? = null
    private val handles = ArrayList<TaskHandle>()
    private val jobs = ArrayList<Job>()

    private val _auth = MutableStateFlow<Auth>(Auth.Starting)
    val auth: StateFlow<Auth> = _auth
    @Volatile var syncState: String = "not started"
        private set
    private val _error = MutableStateFlow<String?>(null)
    val error: StateFlow<String?> = _error
    private val _agentList = MutableStateFlow(AgentList())
    val agentList: StateFlow<AgentList> = _agentList

    private val roomsLock = Any()
    private val recomputeLock = Any()
    private val roomEntries = ArrayList<Room>()
    private val _rooms = MutableStateFlow<Map<String, Room>>(emptyMap())
    /** Which of your agents are in each room (agent rooms only). */
    private val agentMembers = HashMap<String, List<String>>()
    /** Per room checked: your agents in it (empty too), for the agent IDs in [cachedFor]. */
    private val memberCache = HashMap<String, List<String>>()
    @Volatile private var cachedFor: List<String> = emptyList()
    /** Agent rooms' names, avatars and whether they're DMs. */
    private data class RoomMeta(val name: String, val isDm: Boolean, val avatar: String?, val unread: Int = 0)
    private val roomMeta = HashMap<String, RoomMeta>()
    private val timelines = HashMap<String, TimelineModel>()
    private val _agentRooms = MutableStateFlow<Map<String, AgentRoom>>(emptyMap())
    private val _typing = MutableStateFlow<Map<String, List<String>>>(emptyMap())
    /** Who's typing in each agent room (user IDs, you left out). */
    val typing: StateFlow<Map<String, List<String>>> = _typing
    private val typingHandles = HashMap<String, TaskHandle>()
    /** Runs this phone saw live (the desktop's run_history), kept encrypted. */
    private val runHistory = ArrayList<RunRecord>()
    private var historyLoaded = false
    private val _runHistory = MutableStateFlow<List<RunRecord>>(emptyList())
    /** Recorded run history (HistoryScreen reads it). */
    val runHistoryView: StateFlow<List<RunRecord>> = _runHistory
    val agentRooms: StateFlow<Map<String, AgentRoom>> = _agentRooms

    val userId: String get() = runCatching { client?.userId() }.getOrNull().orEmpty()

    fun init(context: Context) {
        if (::appContext.isInitialized) return
        appContext = context.applicationContext
        vault = Vault(appContext)
        Harnesses.load(appContext)
    }

    /** Restore the saved session, if any (the app's start, or a push arriving). */
    suspend fun ensureStarted(): Boolean = io { ensureStartedOnIo() }

    private suspend fun ensureStartedOnIo(): Boolean = startLock.withLock {
        if (client != null) return true
        val saved = vault.load()
        if (saved == null) {
            _auth.value = Auth.SignedOut
            return false
        }
        return try {
            val c = builder(saved.storePassphrase).homeserverUrl(saved.session.homeserverUrl).build()
            c.restoreSession(saved.session)
            started(c)
            true
        } catch (e: Exception) {
            Diag.error("Restoring the session failed", e)
            // Never drop the session over this (the desktop's rule): the
            // error is shown and the next start tries again. SignedOut here
            // showed a real user the sign-in form over a transient keystore
            // failure; RestoreFailed retries without wiping anything.
            _error.value = "Couldn't restore your session: ${e.message}"
            _auth.value = Auth.RestoreFailed
            false
        }
    }

    private fun builder(passphrase: String): ClientBuilder {
        val data = File(appContext.filesDir, "matrix").apply { mkdirs() }
        val cache = File(appContext.cacheDir, "matrix").apply { mkdirs() }
        return ClientBuilder()
            .sqliteStore(SqliteStoreBuilder(data.path, cache.path).passphrase(passphrase))
            .slidingSyncVersionBuilder(SlidingSyncVersionBuilder.DISCOVER_NATIVE)
            .threadsEnabled(true, false)
            .autoEnableCrossSigning(true)
            .setSessionDelegate(object : ClientSessionDelegate {
                override fun retrieveSessionFromKeychain(userId: String): Session =
                    vault.load()?.session ?: throw IllegalStateException("No session")

                override fun saveSessionInKeychain(session: Session) {
                    vault.load()?.let { vault.save(session, it.storePassphrase) }
                }
            })
    }

    suspend fun login(homeserver: String, user: String, password: String) = io { loginOnIo(homeserver, user, password) }

    private suspend fun loginOnIo(homeserver: String, user: String, password: String) = startLock.withLock {
        _error.value = null
        try {
            // A new login starts a new store (the old one belongs to the old device).
            File(appContext.filesDir, "matrix").deleteRecursively()
            File(appContext.cacheDir, "matrix").deleteRecursively()
            val passphrase = Vault.newPassphrase()
            // A full Matrix ID in the homeserver field means its server.
            val server = homeserver.trim().let { if (it.startsWith("@") && ':' in it) it.substringAfter(':') else it }
            val c = builder(passphrase).serverNameOrHomeserverUrl(server).build()
            // "@name" without ":server" isn't a Matrix ID: the server wants just the name then.
            val name = user.trim().let { if (it.startsWith("@") && ':' !in it) it.removePrefix("@") else it }
            c.login(name, password, "fluxchat companion (${android.os.Build.MODEL})", null)
            vault.save(c.session(), passphrase)
            started(c)
        } catch (e: Exception) {
            Diag.error("Login failed", e)
            _error.value = loginError(e, homeserver.trim())
        }
    }

    /** A sign-in failure in words. */
    private fun loginError(e: Exception, server: String): String = when (e) {
        is org.matrix.rustcomponents.sdk.ClientBuildException.ServerUnreachable -> "Couldn't reach $server. Check the address and your connection."
        is org.matrix.rustcomponents.sdk.ClientBuildException.InvalidServerName -> "\"$server\" isn't a homeserver address."
        is org.matrix.rustcomponents.sdk.ClientBuildException -> "That server doesn't look like a Matrix homeserver."
        else -> {
            val m = e.message.orEmpty()
            when {
                "M_FORBIDDEN" in m || "Invalid username or password" in m -> "Wrong username or password."
                "M_INVALID_USERNAME" in m -> "That username isn't valid. Use your name, or your full Matrix ID (@you:server)."
                "M_LIMIT_EXCEEDED" in m -> "Too many attempts. Wait a minute and try again."
                "M_USER_DEACTIVATED" in m -> "This account has been deactivated."
                else -> m.ifEmpty { "Sign-in failed." }
            }
        }
    }

    private suspend fun started(c: Client) {
        client = c
        val sync = c.syncService().finish()
        syncService = sync
        handles += sync.state(object : SyncServiceStateObserver {
            override fun onUpdate(state: SyncServiceState) {
                syncState = state.name.lowercase()
                Diag.note("sync: $syncState")
            }
        })
        sync.start()
        Diag.note("signed in as ${c.userId()} (device ${c.deviceId()}) on ${c.homeserver()}")
        watchRooms(sync)
        startSearch(c)
        loadRunHistory()
        jobs += scope.launch { sweepStalledRuns() }
        jobs += scope.launch { pollAgentList() }
        jobs += combine(_rooms, _agentList) { rooms, list -> rooms to list }
            .debounce(300)
            .onEach { (rooms, _) ->
                // Sync delivers account data with the room list: the agent
                // list may have just arrived (it isn't there at first start).
                loadAgentList()
                refreshAgentRooms(rooms, _agentList.value)
            }
            .launchIn(scope)
        _auth.value = if (needsRecovery(c)) Auth.NeedsRecovery else Auth.Ready
        org.fluxchat.companion.push.Push.registerIfReady(appContext)
    }

    private suspend fun needsRecovery(c: Client): Boolean = runCatching {
        val enc = c.encryption()
        enc.waitForE2eeInitializationTasks()
        enc.verificationState() != VerificationState.VERIFIED &&
            enc.recoveryState().let { it == RecoveryState.ENABLED || it == RecoveryState.INCOMPLETE }
    }.getOrDefault(false)

    suspend fun recover(key: String): Boolean = io { recoverOnIo(key) }

    private suspend fun recoverOnIo(key: String): Boolean = try {
        client?.encryption()?.recover(key.trim())
        Diag.note("recovery key accepted")
        _auth.value = Auth.Ready
        // Messages that failed to decrypt get retried by the SDK once keys arrive.
        true
    } catch (e: Exception) {
        Diag.error("Recovery failed", e)
        _error.value = "That recovery key didn't work: ${e.message}"
        false
    }

    fun skipRecovery() {
        _auth.value = Auth.Ready
    }

    suspend fun logout() = io { logoutOnIo() }

    private suspend fun logoutOnIo() {
        val c = client ?: return
        org.fluxchat.companion.push.Push.unregister(appContext)
        runCatching { syncService?.stop() }
        runCatching { c.logout() }
        stopAll()
        vault.clear()
        File(appContext.filesDir, "matrix").deleteRecursively()
        File(appContext.cacheDir, "matrix").deleteRecursively()
        _agentList.value = AgentList()
        _auth.value = Auth.SignedOut
    }

    private fun stopAll() {
        jobs.forEach { it.cancel() }
        jobs.clear()
        handles.forEach { it.cancel() }
        handles.clear()
        synchronized(timelines) {
            timelines.values.forEach { it.close() }
            timelines.clear()
        }
        synchronized(sessionLists) {
            sessionLists.values.forEach { it.close() }
            sessionLists.clear()
        }
        serverSearch?.close()
        serverSearch = null
        typingHandles.values.forEach { it.cancel() }
        typingHandles.clear()
        _typing.value = emptyMap()
        synchronized(roomsLock) { roomEntries.clear() }
        _rooms.value = emptyMap()
        agentMembers.clear()
        memberCache.clear()
        roomMeta.clear()
        _agentRooms.value = emptyMap()
        synchronized(recomputeLock) {
            runHistory.clear()
            historyLoaded = false
        }
        _runHistory.value = emptyList()
        HistoryStore.clear(appContext)
        client = null
        syncService = null
        notifications = null
    }

    /** Back in the foreground: sync again and look for agent list changes. */
    fun resumed() {
        scope.launch {
            runCatching { syncService?.start() }
            loadAgentList()
        }
    }

    // --- Rooms ---

    private suspend fun watchRooms(sync: SyncService) {
        val list = sync.roomListService().allRooms()
        val result = list.entriesWithDynamicAdapters(500u, object : RoomListEntriesListener {
            override fun onUpdate(roomEntriesUpdate: List<RoomListEntriesUpdate>) {
                synchronized(roomsLock) {
                    for (u in roomEntriesUpdate) applyRoomUpdate(u)
                    _rooms.value = roomEntries.associateBy { it.id() }
                }
            }
        })
        result.controller().setFilter(RoomListEntriesDynamicFilterKind.All(listOf(RoomListEntriesDynamicFilterKind.Joined, RoomListEntriesDynamicFilterKind.NonSpace)))
        handles += result.entriesStream()
    }

    private fun applyRoomUpdate(u: RoomListEntriesUpdate) {
        val e = roomEntries
        when (u) {
            is RoomListEntriesUpdate.Append -> e.addAll(u.values)
            is RoomListEntriesUpdate.Clear -> e.clear()
            is RoomListEntriesUpdate.PushFront -> e.add(0, u.value)
            is RoomListEntriesUpdate.PushBack -> e.add(u.value)
            is RoomListEntriesUpdate.PopFront -> if (e.isNotEmpty()) e.removeAt(0)
            is RoomListEntriesUpdate.PopBack -> if (e.isNotEmpty()) e.removeAt(e.size - 1)
            is RoomListEntriesUpdate.Insert -> e.add(u.index.toInt().coerceAtMost(e.size), u.value)
            is RoomListEntriesUpdate.Set -> if (u.index.toInt() < e.size) e[u.index.toInt()] = u.value
            is RoomListEntriesUpdate.Remove -> if (u.index.toInt() < e.size) e.removeAt(u.index.toInt())
            is RoomListEntriesUpdate.Truncate -> while (e.size > u.length.toInt()) e.removeAt(e.size - 1)
            is RoomListEntriesUpdate.Reset -> {
                e.clear()
                e.addAll(u.values)
            }
        }
    }

    fun room(roomId: String): Room? = _rooms.value[roomId] ?: runCatching { client?.getRoom(roomId) }.getOrNull()

    /** Find the rooms your agents are in and keep their timelines live. */
    private suspend fun refreshAgentRooms(rooms: Map<String, Room>, list: AgentList) {
        val ids = list.agents.map { it.userId }
        if (ids != cachedFor) {
            memberCache.clear()
            cachedFor = ids
        }
        val before = agentMembers.keys.toSet()
        val found = HashMap<String, List<String>>()
        for ((roomId, room) in rooms) {
            val members = memberCache.getOrPut(roomId) { agentsIn(room, ids) }
            if (members.isNotEmpty()) {
                found[roomId] = members
                val info = runCatching { room.roomInfo() }.getOrNull()
                roomMeta[roomId] = RoomMeta(
                    info?.displayName ?: room.displayName() ?: roomId, info?.isDirect ?: false, info?.avatarUrl ?: room.avatarUrl(),
                    (info?.numUnreadMessages ?: 0UL).toInt(),
                )
            }
        }
        synchronized(recomputeLock) {
            agentMembers.clear()
            agentMembers.putAll(found)
        }
        if (found.keys != before) Diag.note("agent rooms: ${found.size} of ${rooms.size} rooms (${found.values.flatten().distinct().joinToString()})")
        // Timelines: open new agent rooms', close the ones that aren't any more.
        for (roomId in found.keys) {
            if (synchronized(timelines) { roomId in timelines }) continue
            val room = rooms[roomId] ?: continue
            runCatching { openRoomTimeline(room) }.onFailure { Diag.error("Timeline for $roomId", it) }
        }
        synchronized(timelines) {
            timelines.keys.filter { it !in found }.forEach {
                timelines.remove(it)?.close()
                typingHandles.remove(it)?.cancel()
            }
        }
        recompute()
    }

    private suspend fun agentsIn(room: Room, agents: List<String>): List<String> {
        if (agents.isEmpty()) return emptyList()
        val heroes = runCatching { room.heroes().map { it.userId } }.getOrDefault(emptyList())
        return agents.filter { a ->
            a in heroes || runCatching {
                val m = room.member(a).membership
                m is MembershipState.Join || m is MembershipState.Invite
            }.getOrDefault(false)
        }
    }

    private suspend fun openRoomTimeline(room: Room) {
        val timeline = room.timelineWithConfiguration(
            TimelineConfiguration(
                focus = TimelineFocus.Live(hideThreadedEvents = false),
                filter = TimelineFilter.All,
                internalIdPrefix = null,
                dateDividerMode = DateDividerMode.DAILY,
                trackReadReceipts = TimelineReadReceiptTracking.DISABLED,
                reportUtds = false,
            ),
        )
        val model = TimelineModel(room.id(), null, timeline, scope)
        synchronized(timelines) { timelines[room.id()] = model }
        val roomId = room.id()
        val me = userId
        typingHandles[roomId] = room.subscribeToTypingNotifications(object : TypingNotificationsListener {
            override fun call(typingUserIds: List<String>) {
                _typing.value = _typing.value + (roomId to typingUserIds.filter { it != me })
            }
        })
        model.start()
        model.messages.debounce(150).onEach { recompute() }.launchIn(scope).also { jobs += it }
        model.refill()
    }

    /** The live timeline of an agent room (thread replies included). */
    fun roomTimeline(roomId: String): TimelineModel? = synchronized(timelines) { timelines[roomId] }

    /** A live timeline for a room that isn't kept open (not an agent room); close it when done. */
    suspend fun openLive(roomId: String): TimelineModel? = openFocused(roomId, null, TimelineFocus.Live(hideThreadedEvents = false))

    /** A thread's own timeline (its whole history; sends go in the thread). Close it when done. */
    suspend fun openThread(roomId: String, root: String): TimelineModel? = openFocused(roomId, root, TimelineFocus.Thread(root))

    private suspend fun openFocused(roomId: String, root: String?, focus: TimelineFocus): TimelineModel? = io { openFocusedOnIo(roomId, root, focus) }

    private suspend fun openFocusedOnIo(roomId: String, root: String?, focus: TimelineFocus): TimelineModel? {
        val room = room(roomId) ?: return null
        val timeline = room.timelineWithConfiguration(
            TimelineConfiguration(
                focus = focus,
                filter = TimelineFilter.All,
                internalIdPrefix = null,
                dateDividerMode = DateDividerMode.DAILY,
                trackReadReceipts = TimelineReadReceiptTracking.DISABLED,
                reportUtds = false,
            ),
        )
        val model = TimelineModel(roomId, root, timeline, scope)
        model.start()
        model.refill()
        return model
    }

    fun roomName(roomId: String): String = roomMeta[roomId]?.name ?: room(roomId)?.displayName() ?: roomId

    fun roomAvatar(roomId: String): String? = roomMeta[roomId]?.avatar ?: room(roomId)?.avatarUrl()

    fun isDm(roomId: String): Boolean = roomMeta[roomId]?.isDm ?: false

    fun agentsInRoom(roomId: String): List<String> = agentMembers[roomId].orEmpty()

    /** Each room's session list, opened on first use and kept while signed in. */
    private val sessionLists = HashMap<String, SessionList>()

    /** A room's sessions (its threads, from the server), loading the first page. */
    suspend fun sessions(roomId: String): SessionList? = io {
        sessionListNow(roomId) ?: openSessionList(roomId)?.also { it.more() }
    }

    /** An open session list, without opening or loading it. */
    fun sessionListNow(roomId: String): SessionList? =
        synchronized(sessionLists) { sessionLists[roomId] }

    /** Open every project's session list (search reads them all). */
    suspend fun sessionsEach(roomIds: List<String>) = io {
        for (roomId in roomIds) openSessionList(roomId)?.more()
        Unit
    }

    // --- Full-text search ---

    /** The server-side full-text search over every session's replies (nil until signed in). */
    @Volatile var serverSearch: ServerSearch? = null
        private set

    private fun startSearch(c: Client) {
        val search = runCatching { ServerSearch(c.searchService()) }
            .onFailure { Diag.error("Opening full-text search", it) }.getOrNull() ?: return
        serverSearch = search
        scope.launch { search.start() }
        // Index the stored history for search, in the background: without it
        // full-text only covers what sync delivered since the last session.
        runCatching { handles += c.runSearchBackfill(uniffi.matrix_sdk.SearchBackfillStrategy.BACKGROUND) }
            .onFailure { Diag.error("Starting the search index backfill", it) }
    }

    private fun openSessionList(roomId: String): SessionList? {
        synchronized(sessionLists) { sessionLists[roomId] }?.let { return it }
        val room = room(roomId) ?: return null
        val list = runCatching { SessionList(roomId, room.threadListService()) }
            .onFailure { Diag.error("Opening the session list of $roomId", it) }.getOrNull() ?: return null
        synchronized(sessionLists) { sessionLists[roomId] = list }
        list.start()
        return list
    }

    private fun recompute() = synchronized(recomputeLock) {
        val list = _agentList.value
        val now = System.currentTimeMillis()
        val out = HashMap<String, AgentRoom>()
        for ((roomId, agents) in agentMembers) {
            val meta = roomMeta[roomId] ?: RoomMeta(roomId, false, null)
            val (name, isDm) = meta.name to meta.isDm
            val msgs = roomTimeline(roomId)?.messages?.value.orEmpty()
            val r = Runs.roomRuns(roomId, msgs, agents, list::harnessOf, isDm, now)
            // A Working run the agent fell silent on is over (agents.rs `stalled`):
            // drop it, so a 👀 Hermes left behind stops holding the room's headline
            // and history's re-opening of a settled record.
            val runs = r.runs.filter { (key, run) ->
                run?.state != RunState.Working ||
                    now - maxOf(run.since, signOfLife(roomId, run.agent, msgs, key.threadRoot, run.since)) <= History.STALE_RUN_MS
            }
            out[roomId] = AgentRoom(
                roomId = roomId,
                name = name,
                isDm = isDm,
                agents = agents,
                runs = runs,
                prompts = r.prompts,
                // Latest activity anywhere in the room: in agent rooms most of it is in session threads.
                last = msgs.lastOrNull { it.isMessage },
                avatar = meta.avatar,
                unread = meta.unread,
            )
            recordRuns(roomId, msgs, runs, now)
        }
        _agentRooms.value = out
    }

    // --- Run history ---

    /** Record what this recompute saw (the desktop's record_run per view). */
    private fun recordRuns(roomId: String, msgs: List<Msg>, runs: Map<ViewKey, RunInfo?>, now: Long) {
        if (!historyLoaded) return
        var changed = false
        for ((key, run) in runs) {
            val rec = run ?: continue
            // The request behind the run: the thread's root, or the room's
            // latest own message at the run's start (lone commands aren't tasks).
            // With slack: a local echo (`~txn`) gets re-timestamped with the
            // server's origin_server_ts when the real event lands.
            val request = key.threadRoot?.let { root -> msgs.firstOrNull { it.id == root } }
                ?: msgs.lastOrNull { it.isOwn && it.threadRoot == null && kotlin.math.abs(it.ts - rec.since) <= REQUEST_MATCH_SLACK_MS }
            val title = key.threadRoot?.let { root ->
                threadTitle(msgs.firstOrNull { it.id == root }, msgs.filter { it.threadRoot == root })
            } ?: "Request in the room"
            if (History.record(runHistory, key, rec, title, request?.body, now)) changed = true
        }
        if (changed) {
            Diag.note("run history: ${runHistory.size} records")
            saveRunHistory()
        }
    }

    /** After sign-in: the encrypted history (runs are only ever recorded live after this). */
    private fun loadRunHistory() {
        synchronized(recomputeLock) {
            val bytes = HistoryStore.load(appContext)
            runHistory.clear()
            if (bytes != null) runHistory.addAll(History.decode(bytes))
            // Older builds recorded lone commands (`!usage`) as runs; newer ones
            // store the request, which is what the desktop prunes on.
            val before = runHistory.size
            runHistory.removeAll { History.isLoneCommand(it.request ?: it.title) }
            historyLoaded = true
            _runHistory.value = runHistory.toList()
            Diag.note("run history: ${runHistory.size} records loaded")
            if (runHistory.size != before) saveRunHistoryLocked()
        }
    }

    private fun saveRunHistory() = synchronized(recomputeLock) { saveRunHistoryLocked() }

    private fun saveRunHistoryLocked() {
        HistoryStore.save(appContext, History.encode(runHistory))
        _runHistory.value = runHistory.toList()
    }

    /** The agent's latest sign of life in [roomId] (the desktop's `last_sign_of_life`):
     *  their own messages for [root]'s view (null: unthreaded), never local echoes. */
    private fun signOfLife(roomId: String, agent: String, msgs: List<Msg>, root: String? = null, since: Long = 0): Long {
        val typingNow = _typing.value[roomId]?.contains(agent) == true
        val lastMsg = msgs.filter {
            !it.isLocal && it.sender == agent && it.threadRoot == root && it.ts >= since
        }.maxOfOrNull { it.ts } ?: 0L
        return if (typingNow) System.currentTimeMillis() else maxOf(lastMsg, since)
    }

    /** A working run silent for STALE_RUN_MS closes as stalled (the desktop's sweep). */
    private suspend fun sweepStalledRuns() {
        while (true) {
            delay(60_000)
            val now = System.currentTimeMillis()
            val changed = synchronized(recomputeLock) {
                if (!historyLoaded) false else History.settleStalled(runHistory, { rec ->
                    val msgs = roomTimeline(rec.room)?.messages?.value.orEmpty()
                    val root = rec.key.removePrefix(rec.room + "::").takeIf { it != rec.key }
                    signOfLife(rec.room, rec.agent, msgs, root, rec.started)
                }, now)
            }
            if (changed) saveRunHistory()
        }
    }

    // --- Push, notification actions ---

    /** Fetches and decrypts the events pushes point at. */
    suspend fun notificationClient(): NotificationClient? {
        notifications?.let { return it }
        val c = client ?: return null
        val sync = syncService ?: return null
        return c.notificationClient(NotificationProcessSetup.SingleProcess(sync)).also { notifications = it }
    }

    /** React to an event (an answer from a notification). */
    suspend fun sendReaction(roomId: String, eventId: String, key: String): Boolean = io { sendReactionOnIo(roomId, eventId, key) }

    private suspend fun sendReactionOnIo(roomId: String, eventId: String, key: String): Boolean = runCatching {
        room(roomId)!!.sendRaw("m.reaction", org.fluxchat.companion.push.ActionReceiver.reaction(eventId, key))
    }.isSuccess

    /** Send text to a room, or a thread in it. */
    suspend fun sendText(roomId: String, thread: String?, text: String, inReplyTo: String?): Boolean = io { sendTextOnIo(roomId, thread, text, inReplyTo) }

    private suspend fun sendTextOnIo(roomId: String, thread: String?, text: String, inReplyTo: String?): Boolean = runCatching {
        room(roomId)!!.sendRaw("m.room.message", org.fluxchat.companion.push.ActionReceiver.text(text, thread, inReplyTo))
    }.isSuccess

    // --- The agent list (account data, shared with desktop fluxchat) ---

    private suspend fun pollAgentList() {
        while (true) {
            loadAgentList()
            delay(AGENT_LIST_POLL_MS)
        }
    }

    suspend fun loadAgentList() = io { loadAgentListOnIo() }

    private suspend fun loadAgentListOnIo() {
        val c = client ?: return
        // The SDK's store has it once sync has delivered it; ask the server
        // otherwise (a new login, or sync not having sent it).
        val stored = runCatching { c.accountData(AgentList.TYPE) }
            .onFailure { Diag.error("Reading the agent list from the store", it) }.getOrNull()
        val json = stored ?: fetchAccountData(c, AgentList.TYPE)
        val list = AgentList.parse(json) ?: AgentList()
        if (list != _agentList.value) {
            _agentList.value = list
            Diag.note("agent list${if (stored == null && json != null) " (from the server)" else ""}: ${list.agents.joinToString { it.userId + "/" + (it.harness ?: "default") }}")
        } else if (json == null && !agentListMissingNoted) {
            agentListMissingNoted = true
            Diag.note("agent list: none saved on the account yet")
        }
    }

    @Volatile private var agentListMissingNoted = false

    /** One global account data event's content from the server (null: not set, or it failed). */
    private suspend fun fetchAccountData(c: Client, type: String): String? = kotlinx.coroutines.withContext(Dispatchers.IO) {
        runCatching {
            val session = c.session()
            val base = session.homeserverUrl.trimEnd('/')
            val user = java.net.URLEncoder.encode(session.userId, "UTF-8")
            val conn = java.net.URL("$base/_matrix/client/v3/user/$user/account_data/$type").openConnection() as java.net.HttpURLConnection
            conn.setRequestProperty("Authorization", "Bearer ${session.accessToken}")
            conn.connectTimeout = 10_000
            conn.readTimeout = 10_000
            try {
                when (conn.responseCode) {
                    200 -> conn.inputStream.bufferedReader().readText()
                    404 -> null
                    else -> throw java.io.IOException("HTTP ${conn.responseCode}")
                }
            } finally {
                conn.disconnect()
            }
        }.onFailure { Diag.error("Fetching $type", it) }.getOrNull()
    }

    /**
     * Start a project: a private room named [name], every agent from
     * [agentList] invited so the room is workable the moment it opens.
     * Encrypted, like every agent room on the homeserver today (the
     * desktop/Element rooms are all m.megolm) — not a new policy. Returns
     * the new room's ID, or null (with [error] set) if it failed; a room
     * that exists but nobody joined is left behind, so the invite retry is
     * possible without a duplicate project.
     */
    suspend fun createProject(name: String, agentList: AgentList): String? = io {
        val c = client ?: return@io null
        val trimmed = name.trim()
        if (trimmed.isEmpty()) return@io null
        val me = runCatching { c.userId() }.getOrNull().orEmpty()
        val invites = org.fluxchat.companion.agents.Projects.inviteList(agentList.agents.map { it.userId }, emptyList(), me)
        try {
            val roomId = c.createRoom(
                // Positional (uniffi-generated): name, topic, encrypted,
                // direct, visibility, preset, invite.
                CreateRoomParameters(
                    trimmed,
                    null,
                    true,
                    false,
                    RoomVisibility.Private,
                    RoomPreset.PRIVATE_CHAT,
                    invites,
                ),
            )
            // Belt and braces: the create request already carries the
            // invites; a per-member invite covers a server that ignored the
            // field. A failure here is reported, not fatal — the room works.
            runCatching { room(roomId) }.getOrNull().let { r ->
                if (r != null) for (id in invites) runCatching { r.inviteUserById(id) }
                    .onFailure { Diag.error("Inviting $id into $roomId", it) }
            }
            Diag.note("project created: $trimmed ($roomId), invited ${invites.joinToString().ifEmpty { "nobody" }}")
            // Pull the room into the list now instead of waiting for the next sync.
            loadAgentListOnIo()
            _rooms.value[roomId]?.let { refreshAgentRooms(_rooms.value, _agentList.value) }
            roomId
        } catch (e: Exception) {
            Diag.error("Creating the project \"$trimmed\"", e)
            _error.value = "Couldn't start your project: ${e.message}"
            null
        }
    }

    /** Leave a project (the room goes on the server without you). */
    suspend fun leaveProject(roomId: String): Boolean = io {
        val r = room(roomId) ?: return@io false
        try {
            r.leave()
            Diag.note("left project $roomId")
            true
        } catch (e: Exception) {
            Diag.error("Leaving $roomId", e)
            _error.value = "Couldn't leave the project: ${e.message}"
            false
        }
    }

    suspend fun saveAgentList(list: AgentList): Boolean = io { saveAgentListOnIo(list) }

    private suspend fun saveAgentListOnIo(list: AgentList): Boolean {
        val c = client ?: return false
        return try {
            c.setAccountData(AgentList.TYPE, list.toJson())
            _agentList.value = list
            true
        } catch (e: Exception) {
            _error.value = "Couldn't save the agent list: ${e.message}"
            false
        }
    }

    /** Settings → Diagnostics: the state of things, to paste into a bug report (no tokens or keys). */
    suspend fun diagnostics(context: Context): String = io { diagnosticsOnIo(context) }

    private suspend fun diagnosticsOnIo(context: Context): String {
        val c = client
        val enc = c?.encryption()
        val rooms = _agentRooms.value
        val version = runCatching { context.packageManager.getPackageInfo(context.packageName, 0).versionName }.getOrNull()
        return buildString {
            appendLine("fluxchat ${version ?: "?"} (${context.packageName}), Android ${android.os.Build.VERSION.RELEASE} (API ${android.os.Build.VERSION.SDK_INT}), ${android.os.Build.MODEL}")
            appendLine("auth: ${_auth.value}")
            if (c != null) {
                appendLine("user: ${runCatching { c.userId() }.getOrNull()} device ${runCatching { c.deviceId() }.getOrNull()}")
                appendLine("homeserver: ${runCatching { c.homeserver() }.getOrNull()}")
                appendLine("sync: $syncState")
                appendLine("encryption: verification ${runCatching { enc?.verificationState() }.getOrNull()}, recovery ${runCatching { enc?.recoveryState() }.getOrNull()}, backup ${runCatching { enc?.backupState() }.getOrNull()}")
            }
            appendLine("rooms: ${_rooms.value.size} joined, ${rooms.size} with agents")
            appendLine("agents: ${_agentList.value.agents.joinToString { it.userId + "/" + (it.harness ?: "default") }.ifEmpty { "none" }}")
            for (r in rooms.values.sortedBy { it.name }) {
                val tl = roomTimeline(r.roomId)?.messages?.value
                val live = r.runs.entries.filter { it.value?.state?.live == true }
                append("  ${r.name}: ${tl?.size ?: "no"} messages loaded, agents ${r.agents.joinToString()}")
                append(", runs ${r.runs.count { it.value != null }} (${live.size} live)")
                appendLine(if (r.prompts.isNotEmpty()) ", ${r.prompts.size} waiting on you" else "")
            }
            appendLine("push: ${org.fluxchat.companion.push.Push.describe(context)}")
            appendLine("pushes received: ${Diag.pushes}${if (Diag.lastPush > 0) ", last ${java.text.DateFormat.getDateTimeInstance().format(java.util.Date(Diag.lastPush))}" else ""}; notifications shown: ${Diag.notified}")
            appendLine()
            appendLine("recent:")
            Diag.recent().forEach { appendLine(it) }
        }
    }

    fun clearError() {
        _error.value = null
    }

    fun launch(block: suspend CoroutineScope.() -> Unit) = scope.launch(block = block)

    /**
     * SDK calls never run on the main thread: the SDK does network work on
     * the thread that polls it, and Android's TLS certificate check
     * (rustls-platform-verifier) throws NetworkOnMainThreadException there.
     */
    private suspend fun <T> io(block: suspend CoroutineScope.() -> T): T = kotlinx.coroutines.withContext(Dispatchers.IO, block)
}
