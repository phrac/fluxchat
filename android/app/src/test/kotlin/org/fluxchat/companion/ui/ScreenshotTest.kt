package org.fluxchat.companion.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material.icons.outlined.SmartToy
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.dp
import com.github.takahirom.roborazzi.captureRoboImage
import org.fluxchat.companion.agents.Harness
import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.agents.Reaction
import org.fluxchat.companion.agents.RunInfo
import org.fluxchat.companion.agents.RunState
import org.fluxchat.companion.agents.ViewKey
import org.fluxchat.companion.matrix.AgentRoom
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import java.io.File

/**
 * The UI with sample data, light and dark, as PNGs in `app/build/screenshots`
 * (record with `-Droborazzi.test.record=true`).
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35], qualifiers = "w411dp-h891dp-xxhdpi")
class ScreenshotTest {
    @get:Rule val compose = createComposeRule()

    private val now = 1_760_000_000_000L
    private val me = "@derek:example.org"
    private val hermes = "@hermes:example.org"
    private val room = "!ops:example.org"
    private val harness = Harness.parse(File("../../assets/agents/hermes.toml").readText())

    private fun msg(id: String, sender: String, minsAgo: Int, body: String, vararg reactions: Pair<String, List<String>>, threadCount: Int = 0) = Msg(
        id, sender, if (sender == me) "Derek" else "Hermes", body, now - minsAgo * 60_000L, isOwn = sender == me,
        reactions = reactions.map { Reaction(it.first, it.second) }, threadCount = threadCount,
    )

    private fun shoot(name: String, dark: Boolean, content: @Composable () -> Unit) {
        if (dark) RuntimeEnvironment.setQualifiers("+night")
        compose.setContent { FluxTheme { Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) { content() } } }
        compose.onRoot().captureRoboImage("build/screenshots/$name-${if (dark) "dark" else "light"}.png")
    }

    @OptIn(ExperimentalMaterial3Api::class)
    @Composable
    private fun Rooms() {
        val waiting = AgentRoom(
            room, "Ops", false, listOf(hermes),
            runs = mapOf(ViewKey(room, "\$t1") to RunInfo(RunState.NeedsYou, now - 4 * 60_000, step = "Running deploy.sh", agent = hermes)),
            last = msg("a", hermes, 1, "Run `./deploy.sh production`?"),
        )
        val working = AgentRoom(
            "!dm:x", "Hermes", true, listOf(hermes),
            runs = mapOf(
                ViewKey("!dm:x") to RunInfo(RunState.Working, now - 3 * 60_000, step = "🔧 Running tests", steps = 6, agent = hermes, costUsd = 0.42),
            ),
            last = msg("b", hermes, 0, "📖 Reading src/main.rs\n🔧 Running tests"),
        )
        val done = AgentRoom(
            "!r:x", "Research", false, listOf(hermes),
            runs = mapOf(ViewKey("!r:x") to RunInfo(RunState.Done, now - 50 * 60_000, agent = hermes)),
            last = msg("c", hermes, 48, "Here's the summary of the three papers you asked about."),
        )
        Scaffold(topBar = {
            LargeTopAppBar(
                title = { Text("Agents") },
                actions = {
                    IconButton(onClick = {}) { Icon(Icons.Outlined.SmartToy, null) }
                    IconButton(onClick = {}) { Icon(Icons.Outlined.Settings, null) }
                },
            )
        }) { pad ->
            Column(Modifier.padding(pad).padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                androidx.compose.foundation.layout.Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    FilterChip(selected = true, onClick = {}, label = { Text("All") })
                    FilterChip(selected = false, onClick = {}, label = { Text("Needs you · 1") }, leadingIcon = { StateDot(RunState.NeedsYou, 8.dp) })
                    FilterChip(selected = false, onClick = {}, label = { Text("Running · 1") }, leadingIcon = { StateDot(RunState.Working, 8.dp) })
                }
                RoomCard(waiting, now, {}, Modifier, msgs = listOf(msg("\$t1", me, 5, "Deploy the new build to production")))
                RoomCard(working, now, {}, Modifier, msgs = emptyList())
                RoomCard(done, now, {}, Modifier, msgs = emptyList())
            }
        }
    }

    @OptIn(ExperimentalMaterial3Api::class)
    @Composable
    private fun Chat() {
        val request = msg("r", me, 6, "Can you clean up the build dir and redeploy?", "👀" to listOf(hermes))
        val steps = msg("s", hermes, 5, "📖 Reading Makefile\n💻 Running make clean\n💻 Running rm -rf build")
        val prompt = msg("p", hermes, 1, "⚠️ Dangerous command: `rm -rf build`\nAllow it?", "✅" to listOf(hermes), "❌" to listOf(hermes))
        Scaffold(topBar = {
            TopAppBar(
                navigationIcon = { IconButton(onClick = {}) { Icon(Icons.AutoMirrored.Filled.ArrowBack, null) } },
                title = {
                    Column {
                        Text("Hermes", style = MaterialTheme.typography.titleMedium)
                        Text("Needs you · 6m", style = MaterialTheme.typography.labelMedium, color = RunState.NeedsYou.color())
                    }
                },
            )
        }) { pad ->
            Column(Modifier.padding(pad)) {
                Column(Modifier.weight(1f)) {
                    DaySeparator(now)
                    MessageRow(request, false, true, false, false, emptyList(), null, {}, {}, null, null, now)
                    MessageRow(steps, false, false, true, false, emptyList(), null, {}, {}, null, null, now)
                    MessageRow(prompt, true, true, true, false, listOf("✅" to "Approve once", "❌" to "Deny"), null, {}, {}, null, null, now)
                }
                RunCard(RunInfo(RunState.NeedsYou, now - 6 * 60_000, step = "💻 Running rm -rf build", steps = 3, agent = hermes), now, true, {}, null)
                Composer(harness, working = false) { _, _ -> }
            }
        }
    }

    @Composable
    private fun Sessions() {
        val cards = listOf(
            SessionCardInfo("a", "Deploy the new build to production", null, null, 0, now - 20_000, null, own = true),
            SessionCardInfo(
                "b", "Clean up the build dir and redeploy", null, null, 3, now - 2 * 60_000,
                RunInfo(RunState.Working, now - 3 * 60_000, step = "💻 Running make clean", steps = 4, agent = hermes), own = true,
            ),
            SessionCardInfo(
                "c", "Which of the three papers covers sparse attention?", "Only the second one: it compares block-sparse and sliding-window attention on long documents.",
                "Hermes", 5, now - 50 * 60_000, RunInfo(RunState.Done, now - 55 * 60_000, agent = hermes, costUsd = 0.12), own = true,
            ),
        )
        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            for (c in cards.asReversed()) SessionCard(c, now) {}
            Composer(harness, working = false, onSend = { _, _ -> }, placeholder = "Start a new session", canAttach = true)
        }
    }

    @Composable
    private fun Transcript() {
        val request = msg("r", me, 6, "Can you clean up the build dir and redeploy?", "👀" to listOf(hermes))
        val steps = msg("s", hermes, 5, "📖 Reading `Makefile`\n💻 Running `make clean`\n🔍 Searching for stale artifacts\n💻 Running `rm -rf build`")
        val answer = msg("a", hermes, 4, "Cleaned up. **build/** was 2.1 GB of stale artifacts from the old toolchain.\n\nBefore redeploying, one thing needs your approval:")
        val prompt = msg("p", hermes, 1, "⚠️ Dangerous command: `./deploy.sh production`\nAllow it?", "✅" to listOf(hermes), "❌" to listOf(hermes))
        val report = msg("f", hermes, 3, "The cleanup log:").copy(
            file = org.fluxchat.companion.agents.FileRef("{}", "cleanup-report.pdf", "application/pdf", 182_000, org.fluxchat.companion.agents.FileKind.File),
        )
        val hidden: (Reaction) -> Boolean = { it.users == listOf(hermes) && it.emoji in setOf("👀", "✅", "❌") }
        Column {
            Column(Modifier.weight(1f)) {
                TranscriptRow(request, null, false, hidden, emptyList(), null, false, {}, {})
                TranscriptRow(steps, request, true, hidden, emptyList(), null, true, {}, {})
                TranscriptRow(answer, steps, true, hidden, emptyList(), null, false, {}, {})
                TranscriptRow(report, answer, true, hidden, emptyList(), null, false, {}, {})
                TranscriptRow(prompt, report, true, hidden, listOf("✅" to "Approve once", "❌" to "Deny"), null, false, {}, {})
            }
            Composer(harness, working = true, canAttach = true) { _, _ -> }
        }
    }

    @Composable
    private fun FinalAnswer() {
        val approval = msg("ap", hermes, 9, "⚠️ **Approval needed:** `execute_code`\n```python\nr = terminal(\"curl -s https://photos.example/api\")\nprint(r[\"output\"][:500])\n```\nReply `!approve session` to approve this pattern for the session.", "✅" to listOf(me))
        val answer = msg("fa", hermes, 2, "Done, both verified against the API:\n\n- **Uploaded** to Immich as `tuxedo_trophy_bass.jpg` — asset `46cb21da…`, visibility read back as `timeline` (not silently archived), dated today.\n- **Created album \"AI Generated\"** (`2a63b5b9…`) with the photo in it — read back `assetCount: 1`.\n\n## Next\n1. Any future AI edits can go in that album too.\n2. Say the word and I'll share it.\n\n```bash\ncurl -s https://photos.example/api/albums | jq '.[] | .albumName'\n```")
        val hidden: (Reaction) -> Boolean = { false }
        Column {
            TranscriptRow(approval, null, true, hidden, answered = "✅ Approve once", prompt = emptyList(), question = null, live = false, onReact = {}, onAnswer = {})
            TranscriptRow(answer, approval, true, hidden, prompt = emptyList(), question = null, live = false, onReact = {}, onAnswer = {})
        }
    }

    @Test fun answerLight() = shoot("answer", false) { FinalAnswer() }
    @Test fun answerDark() = shoot("answer", true) { FinalAnswer() }
    @Test fun transcriptLight() = shoot("transcript", false) { Transcript() }
    @Test fun transcriptDark() = shoot("transcript", true) { Transcript() }
    @Test fun sessionsLight() = shoot("sessions", false) { Sessions() }
    @Test fun sessionsDark() = shoot("sessions", true) { Sessions() }
    @Test fun roomsLight() = shoot("rooms", false) { Rooms() }
    @Test fun roomsDark() = shoot("rooms", true) { Rooms() }
    @Test fun chatLight() = shoot("chat", false) { Chat() }
    @Test fun chatDark() = shoot("chat", true) { Chat() }
}
