package org.fluxchat.companion

import android.content.Context
import android.content.Intent
import android.net.Uri
import java.net.HttpURLConnection
import java.net.URL
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.withContext
import org.json.JSONObject

/**
 * The app discovers its own updates: fetch the newest GitHub Release, compare
 * its tag to BuildConfig.VERSION_NAME, and hold the result for the Projects
 * bar chip. Checks are rate-limited to once a day; any failure is silent to
 * the user (Diag.error only).
 */
object Update {
    private const val PREFS = "fluxchat"
    private const val LAST_CHECK = "update_last_check"
    private const val CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000L
    private const val LATEST_URL = "https://api.github.com/repos/phrac/fluxchat/releases/latest"

    private lateinit var appContext: Context

    private val _available = MutableStateFlow<String?>(null)
    /** Latest release tag newer than the running version, else null. */
    val available: StateFlow<String?> = _available

    fun init(context: Context) {
        if (::appContext.isInitialized) return
        appContext = context.applicationContext
    }

    /**
     * GET releases/latest unless the last attempt was under a day ago
     * (failures count too, so an offline phone retries tomorrow, not per
     * launch). Force bypasses the window for pull-to-refresh.
     */
    suspend fun check(force: Boolean = false) {
        val prefs = appContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val last = prefs.getLong(LAST_CHECK, 0L)
        if (!force && System.currentTimeMillis() - last < CHECK_INTERVAL_MS) return
        prefs.edit().putLong(LAST_CHECK, System.currentTimeMillis()).apply()
        val tag = runCatching { fetchLatestTag() }
            .onFailure { Diag.error("update check (github releases)", it) }
            .getOrNull() ?: return
        val name = tag.removePrefix("v")
        if (isNewer(BuildConfig.VERSION_NAME, name)) {
            _available.value = name
            Diag.note("update: $name available (running ${BuildConfig.VERSION_NAME})")
        }
    }

    /** Open the release page (where the APK asset lives). */
    fun openRelease(context: Context) {
        val tag = _available.value ?: return
        runCatching {
            context.startActivity(
                Intent(Intent.ACTION_VIEW, Uri.parse("https://github.com/phrac/fluxchat/releases/tag/v$tag"))
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            )
        }.onFailure { Diag.error("opening the release page", it) }
    }

    private suspend fun fetchLatestTag(): String = withContext(Dispatchers.IO) {
        val conn = URL(LATEST_URL).openConnection() as HttpURLConnection
        conn.connectTimeout = 5000
        conn.readTimeout = 5000
        conn.setRequestProperty("Accept", "application/vnd.github+json")
        try {
            if (conn.responseCode != 200) error("HTTP ${conn.responseCode}")
            JSONObject(conn.inputStream.bufferedReader().readText()).getString("tag_name")
        } finally {
            conn.disconnect()
        }
    }

    /**
     * Dot-separated versions compared part by part (numeric when both sides
     * are numeric, else lexicographic); a longer version wins ties, so
     * 0.3 > 0.2.9 and 0.2.2.1 > 0.2.2.
     */
    fun isNewer(current: String, latest: String): Boolean {
        val a = current.split('.')
        val b = latest.split('.')
        for (i in 0 until maxOf(a.size, b.size)) {
            val x = a.getOrNull(i)
            val y = b.getOrNull(i)
            if (y == null) return false // current has more parts: current is newer
            if (x == null) return true  // latest has more parts
            val c = if (x.all { it.isDigit() } && y.all { it.isDigit() }) {
                x.toLong().compareTo(y.toLong())
            } else {
                x.compareTo(y)
            }
            if (c != 0) return c < 0
        }
        return false // equal
    }
}
