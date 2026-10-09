package org.fluxchat.companion.push

import android.app.Activity
import android.content.Context
import android.os.Build
import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.withContext
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
    private const val TAG = "fluxchat-push"
    private const val PREFS = "push"
    private const val FALLBACK_GATEWAY = "https://matrix.gateway.unifiedpush.org/_matrix/push/v1/notify"
    private const val GATEWAY_PATH = "/_matrix/push/v1/notify"

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

    private fun prefs(context: Context) = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

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
                prefs(activity).edit().putBoolean("enabled", true).apply()
                UnifiedPush.register(activity, messageForDistributor = "fluxchat agents")
            } else {
                _status.value = Status.NoDistributor
            }
        }
    }

    /** Use this distributor (a choice in Settings). */
    fun use(context: Context, distributor: String) {
        _status.value = Status.Registering
        UnifiedPush.saveDistributor(context, distributor)
        prefs(context).edit().putBoolean("enabled", true).apply()
        UnifiedPush.register(context, messageForDistributor = "fluxchat agents")
    }

    /** After sign-in / on start: renew the registration, and the pusher if we have an endpoint. */
    suspend fun registerIfReady(context: Context) {
        val p = prefs(context)
        if (!p.getBoolean("enabled", false)) {
            _status.value = Status.Off
            return
        }
        val distributor = UnifiedPush.getAckDistributor(context)
        if (distributor == null) {
            _status.value = if (distributors(context).isEmpty()) Status.NoDistributor else Status.Off
            return
        }
        p.getString("endpoint", null)?.let { setPusher(context, it) }
        // Checks the connection; a new endpoint comes back through the service.
        UnifiedPush.register(context, messageForDistributor = "fluxchat agents")
    }

    /** The distributor gave us an endpoint (new or changed). */
    suspend fun newEndpoint(context: Context, endpoint: String) {
        prefs(context).edit().putString("endpoint", endpoint).apply()
        if (Matrix.ensureStarted()) setPusher(context, endpoint)
    }

    private suspend fun setPusher(context: Context, endpoint: String) {
        val client = Matrix.client ?: return
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
            val old = prefs(context).getString("pushed_endpoint", null)
            if (old != null && old != endpoint) {
                runCatching { client.deletePusher(PusherIdentifiers(old, context.packageName)) }
            }
            prefs(context).edit().putString("pushed_endpoint", endpoint).putString("gateway", gateway).apply()
            _status.value = Status.On(UnifiedPush.getAckDistributor(context) ?: "?", gateway)
            org.fluxchat.companion.Diag.note("pusher set: gateway $gateway")
        } catch (e: Exception) {
            org.fluxchat.companion.Diag.error("Setting the pusher failed", e)
            _status.value = Status.Failed("Couldn't register with your homeserver: ${e.message}")
        }
    }

    /** For Diagnostics: status, distributor, gateway, and the endpoint's host only (its path is a secret topic). */
    fun describe(context: Context): String {
        val p = prefs(context)
        val endpoint = p.getString("endpoint", null)?.let { runCatching { URL(it).host }.getOrNull() ?: "?" }
        return listOf(
            "status ${_status.value}",
            "enabled ${p.getBoolean("enabled", false)}",
            "distributor ${UnifiedPush.getAckDistributor(context) ?: "none"} (installed: ${distributors(context).joinToString().ifEmpty { "none" }})",
            "endpoint host ${endpoint ?: "none"}",
            "gateway ${p.getString("gateway", null) ?: "none"}",
            "pusher set ${p.getString("pushed_endpoint", null) != null}",
        ).joinToString(", ")
    }

    fun failed(reason: String) {
        org.fluxchat.companion.Diag.error("push: $reason")
        _status.value = Status.Failed(reason)
    }

    /** The distributor dropped us (or we turned push off). */
    suspend fun forget(context: Context) {
        val p = prefs(context)
        val endpoint = p.getString("pushed_endpoint", null)
        if (endpoint != null) {
            runCatching { Matrix.client?.deletePusher(PusherIdentifiers(endpoint, context.packageName)) }
        }
        p.edit().remove("endpoint").remove("pushed_endpoint").remove("gateway").apply()
        if (_status.value !is Status.Failed) _status.value = Status.Off
    }

    /** Turn push off (Settings, sign out). */
    suspend fun unregister(context: Context) {
        prefs(context).edit().putBoolean("enabled", false).apply()
        forget(context)
        runCatching { UnifiedPush.unregister(context) }
        _status.value = Status.Off
    }

    /**
     * The Matrix push gateway for an endpoint: the endpoint's server if it
     * runs one (ntfy does: `GET /_matrix/push/v1/notify` says
     * `{"unifiedpush":{"gateway":"matrix"}}`), else unifiedpush.org's.
     */
    private suspend fun gatewayFor(endpoint: String): String = withContext(Dispatchers.IO) {
        runCatching {
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
        }.getOrNull() ?: FALLBACK_GATEWAY
    }
}
