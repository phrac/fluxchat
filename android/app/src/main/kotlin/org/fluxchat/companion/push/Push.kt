package org.fluxchat.companion.push

import android.app.Activity
import android.content.Context
import android.os.Build
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.fluxchat.companion.Diag
import org.fluxchat.companion.matrix.Matrix
import org.json.JSONObject
import org.matrix.rustcomponents.sdk.HttpPusherData
import org.matrix.rustcomponents.sdk.PushFormat
import org.matrix.rustcomponents.sdk.PusherIdentifiers
import org.matrix.rustcomponents.sdk.PusherKind
import org.unifiedpush.android.connector.UnifiedPush
import java.net.HttpURLConnection
import java.net.URL
import java.util.Locale

/**
 * Push through UnifiedPush: a distributor app (ntfy) hands us an endpoint,
 * the homeserver gets a pusher that sends to it through a Matrix push
 * gateway (ntfy's own, else unifiedpush.org's), and messages come back to
 * [FluxPushService].
 */
object Push {
    private const val PREFS = "push"
    private const val FALLBACK_GATEWAY = "https://matrix.gateway.unifiedpush.org/_matrix/push/v1/notify"
    private const val GATEWAY_PATH = "/_matrix/push/v1/notify"
    private const val MESSAGE = "fluxchat agents"
    /** A distributor that hasn't answered a registration by now isn't going to by itself. */
    private const val REGISTER_WAIT_MS = 30_000L

    sealed interface Status {
        data object Off : Status
        /** No distributor app installed (ntfy). */
        data object NoDistributor : Status
        data object Registering : Status
        data class On(val distributor: String, val gateway: String) : Status
        data class Failed(val reason: String) : Status
    }

    private val _status = MutableStateFlow<Status>(Status.Off)
    val status: StateFlow<Status> = _status

    /**
     * What is saved about push. Two different things live here: what *you*
     * chose ([enabled], through which [distributor]) and what is currently
     * registered ([endpoint], the pusher on the server). Signing out ends
     * the registration, not the choice.
     */
    internal data class Saved(
        val enabled: Boolean = false,
        val distributor: String? = null,
        val endpoint: String? = null,
        /** The endpoint the server's pusher points at. */
        val pushedEndpoint: String? = null,
        val gateway: String? = null,
    ) {
        /**
         * Signed out: the pusher and the endpoint go, the choice stays, so
         * the next sign-in registers again. It used to turn push off, and
         * after signing back in nothing arrived until you found the switch.
         */
        fun signedOut() = copy(endpoint = null, pushedEndpoint = null, gateway = null)

        /** You turned push off in Settings. */
        fun turnedOff() = signedOut().copy(enabled = false)
    }

    /** What to do about push when a session starts. */
    internal sealed interface Plan {
        data object Off : Plan
        data object NoDistributor : Plan
        /** Several distributors and none of them the one chosen: Settings asks. */
        data object Pick : Plan
        /** Registered: check the connection, put the pusher back. */
        data class Renew(val distributor: String) : Plan
        /** Chosen but not registered (signed out and back in): register again. */
        data class Reconnect(val distributor: String) : Plan
    }

    /** [acked]: the distributor UnifiedPush has a live registration with; [installed]: the ones on the phone. */
    internal fun plan(saved: Saved, acked: String?, installed: List<String>): Plan = when {
        !saved.enabled -> Plan.Off
        acked != null -> Plan.Renew(acked)
        installed.isEmpty() -> Plan.NoDistributor
        saved.distributor != null && saved.distributor in installed -> Plan.Reconnect(saved.distributor)
        installed.size == 1 -> Plan.Reconnect(installed.single())
        else -> Plan.Pick
    }

    private fun prefs(context: Context) = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    private fun load(context: Context): Saved = prefs(context).let { p ->
        Saved(
            p.getBoolean("enabled", false), p.getString("distributor", null), p.getString("endpoint", null),
            p.getString("pushed_endpoint", null), p.getString("gateway", null),
        )
    }

    private fun store(context: Context, s: Saved) {
        val e = prefs(context).edit().putBoolean("enabled", s.enabled)
        for ((key, value) in listOf("distributor" to s.distributor, "endpoint" to s.endpoint, "pushed_endpoint" to s.pushedEndpoint, "gateway" to s.gateway)) {
            if (value != null) e.putString(key, value) else e.remove(key)
        }
        e.apply()
    }

    fun distributors(context: Context): List<String> = UnifiedPush.getDistributors(context).filter { it != context.packageName }

    /** Turn push on: the distributor in use, else the system's default (asks when there are several). */
    fun enable(activity: Activity) {
        if (distributors(activity).isEmpty()) {
            _status.value = Status.NoDistributor
            return
        }
        _status.value = Status.Registering
        UnifiedPush.tryUseCurrentOrDefaultDistributor(activity) { ok ->
            if (ok) {
                Diag.note("push: enabled via the default distributor")
                val app = activity.applicationContext
                store(app, load(app).copy(enabled = true, distributor = UnifiedPush.getSavedDistributor(app)))
                UnifiedPush.register(app, messageForDistributor = MESSAGE)
                expectEndpoint()
            } else {
                Diag.note("push: enable refused — no distributor to pick")
                _status.value = Status.NoDistributor
            }
        }
    }

    /** Use this distributor (a choice in Settings). */
    fun use(context: Context, distributor: String) {
        Diag.note("push: using distributor $distributor")
        connect(context.applicationContext, distributor)
    }

    private fun connect(context: Context, distributor: String) {
        _status.value = Status.Registering
        UnifiedPush.saveDistributor(context, distributor)
        store(context, load(context).copy(enabled = true, distributor = distributor))
        UnifiedPush.register(context, messageForDistributor = MESSAGE)
        expectEndpoint()
    }

    @Volatile private var waiting: Job? = null

    /**
     * "Connecting…" must end. A distributor that was never opened, or that
     * the system keeps from starting, answers nothing at all — the switch
     * sat on "Connecting…" for good.
     */
    private fun expectEndpoint() {
        waiting?.cancel()
        waiting = Matrix.scope.launch {
            delay(REGISTER_WAIT_MS)
            if (_status.value == Status.Registering) failed("The push app didn't answer. Open it once, then turn push on again.")
        }
    }

    /** After sign-in / on start: renew the registration, and the pusher if we have an endpoint. */
    suspend fun registerIfReady(context: Context) = withContext<Unit>(Dispatchers.IO) {
        val saved = load(context)
        when (val plan = plan(saved, UnifiedPush.getAckDistributor(context), distributors(context))) {
            Plan.Off -> _status.value = Status.Off
            Plan.NoDistributor -> {
                Diag.note("push: on, but no distributor app is installed")
                _status.value = Status.NoDistributor
            }
            Plan.Pick -> {
                Diag.note("push: on, but the distributor it used is gone — pick one in Settings")
                _status.value = Status.Off
            }
            is Plan.Renew -> {
                if (saved.distributor != plan.distributor) store(context, saved.copy(distributor = plan.distributor))
                if (saved.endpoint == null) {
                    Diag.note("push: waiting on an endpoint from ${plan.distributor}")
                    _status.value = Status.Registering
                    expectEndpoint()
                } else {
                    setPusher(context, saved.endpoint)
                }
                // Checks the connection; a new endpoint comes back through the service.
                UnifiedPush.register(context, messageForDistributor = MESSAGE)
            }
            is Plan.Reconnect -> {
                Diag.note("push: registering again with ${plan.distributor}")
                connect(context, plan.distributor)
            }
        }
    }

    /** The distributor gave us an endpoint (new or changed). */
    suspend fun newEndpoint(context: Context, endpoint: String) = withContext<Unit>(Dispatchers.IO) {
        waiting?.cancel()
        val saved = load(context)
        store(context, saved.copy(endpoint = endpoint, distributor = UnifiedPush.getAckDistributor(context) ?: saved.distributor))
        when {
            // Turned off while the registration was on its way: no pusher for it.
            !saved.enabled -> Diag.note("push: an endpoint arrived with push turned off, not used")
            // The distributor confirming the endpoint the pusher already has
            // (every start asks it to): nothing to set a second time.
            endpoint == saved.pushedEndpoint && _status.value is Status.On -> Unit
            // No session yet: the pusher goes in when registerIfReady runs after sign-in.
            !Matrix.ensureStarted() -> Diag.note("push: endpoint saved, no session to set the pusher with")
            else -> setPusher(context, endpoint)
        }
    }

    private suspend fun setPusher(context: Context, endpoint: String) {
        val client = Matrix.client ?: return Diag.note("push: no session to set the pusher with")
        try {
            val gateway = gatewayFor(endpoint)
            client.setPusher(
                identifiers = PusherIdentifiers(pushkey = endpoint, appId = context.packageName),
                kind = PusherKind.Http(HttpPusherData(url = gateway, format = PushFormat.EVENT_ID_ONLY, defaultPayload = null)),
                appDisplayName = "fluxchat",
                deviceDisplayName = Build.MODEL ?: "Android",
                profileTag = null,
                lang = Locale.getDefault().toLanguageTag(),
                append = false,
            )
            val old = load(context).pushedEndpoint
            if (old != null && old != endpoint) {
                // Left behind, the server keeps pushing every event to an endpoint nobody reads.
                try {
                    client.deletePusher(PusherIdentifiers(old, context.packageName))
                } catch (e: Exception) {
                    Diag.error("Removing the pusher for the previous endpoint", e)
                }
            }
            store(context, load(context).copy(pushedEndpoint = endpoint, gateway = gateway))
            _status.value = Status.On(UnifiedPush.getAckDistributor(context) ?: "?", gateway)
            Diag.note("pusher set: gateway $gateway")
        } catch (e: Exception) {
            Diag.error("Setting the pusher failed", e)
            _status.value = Status.Failed("Couldn't register with your server: ${e.message}")
        }
    }

    /** For Diagnostics: status, distributor, gateway, and the endpoint's host only (its path is a secret topic). */
    fun describe(context: Context): String {
        val s = load(context)
        val endpoint = s.endpoint?.let { runCatching { URL(it).host }.getOrNull() ?: "?" }
        return listOf(
            "status ${_status.value}",
            "enabled ${s.enabled}",
            "distributor ${UnifiedPush.getAckDistributor(context) ?: "none"} (chosen: ${s.distributor ?: "none"}; installed: ${distributors(context).joinToString().ifEmpty { "none" }})",
            "endpoint host ${endpoint ?: "none"}",
            "gateway ${s.gateway ?: "none"}",
            "pusher set ${s.pushedEndpoint != null}",
        ).joinToString(", ")
    }

    fun failed(reason: String) {
        Diag.error("push: $reason")
        _status.value = Status.Failed(reason)
    }

    /**
     * The registration is over (the distributor dropped us, signing out,
     * turning push off): the server's pusher and the saved endpoint go.
     * Uses the session as it stands — it is called from inside sign-out,
     * which must not be asked to start one.
     */
    suspend fun forget(context: Context) = withContext<Unit>(Dispatchers.IO) {
        val saved = load(context)
        val endpoint = saved.pushedEndpoint
        if (endpoint != null) {
            val client = Matrix.client
            if (client == null) {
                Diag.note("push: no session to remove the pusher with (the server drops it with the session)")
            } else {
                try {
                    client.deletePusher(PusherIdentifiers(endpoint, context.packageName))
                    Diag.note("pusher removed")
                } catch (e: Exception) {
                    Diag.error("Removing the pusher (the server may keep pushing to a dead endpoint)", e)
                }
            }
        }
        store(context, load(context).signedOut())
        if (_status.value !is Status.Failed) _status.value = Status.Off
    }

    /**
     * Signing out (`Matrix.logout`): the pusher, the endpoint and the
     * distributor's registration go; that you want push stays, and the next
     * sign-in registers again ([Plan.Reconnect]).
     */
    suspend fun unregister(context: Context) {
        drop(context)
    }

    /** You turned push off (Settings). */
    suspend fun disable(context: Context) {
        Diag.note("push: turned off")
        // Off first (an endpoint arriving meanwhile is not used); the pusher's
        // endpoint stays on record until `forget` has removed the pusher.
        val saved = load(context)
        store(context, saved.turnedOff().copy(pushedEndpoint = saved.pushedEndpoint))
        drop(context)
    }

    private suspend fun drop(context: Context) = withContext<Unit>(Dispatchers.IO) {
        waiting?.cancel()
        forget(context)
        // Removes the registration and, with none left, the connector's saved distributor.
        try {
            UnifiedPush.unregister(context)
        } catch (e: Exception) {
            Diag.error("Unregistering from the push distributor", e)
        }
        _status.value = Status.Off
    }

    /**
     * The Matrix push gateway for an endpoint: the endpoint's server if it
     * runs one (ntfy does: `GET /_matrix/push/v1/notify` says
     * `{"unifiedpush":{"gateway":"matrix"}}`), else unifiedpush.org's.
     */
    private suspend fun gatewayFor(endpoint: String): String = withContext(Dispatchers.IO) {
        val probed = runCatching {
            val u = URL(endpoint)
            val candidate = URL(u.protocol, u.host, u.port, GATEWAY_PATH).toString()
            val conn = URL(candidate).openConnection() as HttpURLConnection
            conn.connectTimeout = 5000
            conn.readTimeout = 5000
            try {
                if (conn.responseCode != 200) return@runCatching null
                val body = conn.inputStream.bufferedReader().readText()
                val ok = JSONObject(body).optJSONObject("unifiedpush")?.optString("gateway") == "matrix"
                if (ok) candidate else null
            } finally {
                conn.disconnect()
            }
        }
        probed.getOrNull()?.let { return@withContext it }
        // A failed probe still means push works via the fallback; say which path we took.
        probed.exceptionOrNull()?.let { Diag.note("push: gateway probe failed (${it.javaClass.simpleName}: ${it.message})") }
        Diag.note("push: no Matrix gateway at the endpoint's server — using unifiedpush.org's")
        FALLBACK_GATEWAY
    }
}
