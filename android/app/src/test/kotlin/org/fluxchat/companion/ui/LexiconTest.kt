package org.fluxchat.companion.ui

import java.io.File
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The app speaks "projects, sessions, agents" — Matrix is plumbing, not product.
 * User-visible strings (and accessibility labels) in the UI must not leak the
 * protocol: no "room", "matrix", "homeserver", "element", "encrypted", "E2E".
 * Plumbing lives in the settings screens (allow-listed below) and diagnostics.
 * A deliberate exception carries a `// lexicon:ok` comment on the line.
 */
class LexiconTest {
    private companion object {
        // Settings is the honest drawer for account plumbing; diagnostics copies
        // SDK state verbatim into bug reports.
        val ALLOWED_FILES = setOf("AgentsScreen.kt", "DiagnosticsScreen.kt")
        val BANNED = listOf("room", "rooms", "matrix", "homeserver", "element", "encrypted", "encryption", "e2e")
        val STRING = Regex("\"([^\"\\\\]*(?:\\\\.[^\"\\\\]*)*)\"")
    }

    private fun uiDir(): File {
        var dir = File(System.getProperty("user.dir"))
        val candidate = File(dir, "src/main/kotlin/org/fluxchat/companion/ui")
        check(candidate.isDirectory) { "run from the app module: ${dir.absolutePath}" }
        return candidate
    }

    @Test
    fun userVisibleStringsDontLeakTheProtocol() {
        val offenders = ArrayList<String>()
        for (file in uiDir().listFiles()!!.filter { it.extension == "kt" }.sortedBy { it.name }) {
            if (file.name in ALLOWED_FILES) continue
            file.readLines().forEachIndexed { i, line ->
                if ("lexicon:ok" in line) return@forEachIndexed
                STRING.findAll(line).forEach { m ->
                    // Drop interpolated expressions: identifiers inside ${...} are code, not copy.
                    val text = m.groupValues[1].replace(Regex("\\$\\{[^}]*\\}"), "").replace(Regex("\\$[A-Za-z_][A-Za-z0-9_]*"), "")
                    val words = Regex("[a-zA-Z0-9]+").findAll(text).map { it.value.lowercase() }.toSet()
                    val hit = BANNED.firstOrNull { it in words }
                    if (hit != null) offenders += "${file.name}:${i + 1}: \"$hit\" in \"${m.groupValues[1].take(60)}\""
                }
            }
        }
        assertTrue("Protocol vocabulary leaked into user-visible strings:\n" + offenders.joinToString("\n"), offenders.isEmpty())
    }
}
