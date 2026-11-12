package org.fluxchat.companion.ui

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Looper
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Download
import androidx.compose.material.icons.outlined.Share
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.dp
import com.github.takahirom.roborazzi.captureRoboImage
import org.fluxchat.companion.agents.FileKind
import org.fluxchat.companion.agents.FileRef
import org.fluxchat.companion.agents.Media
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import org.robolectric.shadows.ShadowToast
import java.io.ByteArrayOutputStream
import java.io.File
import java.security.MessageDigest

/**
 * The secondary screens' pieces on a small phone (360 dp, where the Projects
 * bar overflowed), as PNGs in `app/build/screenshots` like [ScreenshotTest]'s
 * (record with `-Droborazzi.test.record=true`), and what they do when tapped.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35], qualifiers = "w360dp-h740dp-xhdpi")
class SecondaryScreensTest {
    @get:Rule val compose = createComposeRule()

    private fun show(content: @Composable () -> Unit) =
        compose.setContent { FluxTheme { Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) { content() } } }

    private fun shoot(name: String, content: @Composable () -> Unit) {
        show(content)
        compose.onRoot().captureRoboImage("build/screenshots/$name.png")
    }

    @OptIn(ExperimentalMaterial3Api::class)
    @Composable
    private fun Bars(go: (Screen) -> Unit = {}) {
        Column {
            LargeTopAppBar(title = { Text("Projects") }, actions = { ProjectsActions(update = true, onUpdate = {}, onNew = {}, go = go) })
            HorizontalDivider()
            // Scrolled, the bar is one row: the title beside the actions.
            TopAppBar(title = { Text("Projects") }, actions = { ProjectsActions(update = true, onUpdate = {}, onNew = {}, go = {}) })
            HorizontalDivider()
            TopAppBar(title = { Text("Projects") }, actions = { ProjectsActions(update = false, onUpdate = {}, onNew = {}, go = {}) })
        }
    }

    @Test fun projectsBarAt360() = shoot("sweep-projects-bar-360") { Bars() }

    @Test
    fun theMenuHoldsWhatLeftTheBar() {
        val went = ArrayList<Screen>()
        show { Bars(went::add) }
        // Settings is always reachable: as the fifth button it was pushed off a 360 dp bar.
        compose.onNodeWithText("Update available", useUnmergedTree = true).assertDoesNotExist()
        compose.onAllNodesWithContentDescription("More")[0].performClick()
        compose.onNodeWithText("Run history").assertIsDisplayed()
        compose.onNodeWithText("Your agents").assertIsDisplayed()
        compose.onNodeWithText("Settings").performClick()
        assertEquals(listOf<Screen>(Screen.Settings), went)
        compose.onNodeWithText("Run history").assertDoesNotExist()
    }

    private val markdown = """
        Status of the three services:

        | Service | State | Notes |
        |---------|:-----:|-------|
        | `api` | **up** | [dashboard](https://example.com/d) |
        | `worker | batch` | down | restarts with `a \| b` |
        | web | up | |

        Name | Took
        --- | ---
        build | 41 s
        test | 3 min

        1. Build
            - compile
                - `kotlinc`
            - link
        2. Ship **[PR 12](https://example.com/pr/12)** with _care_: **fixed `foo()`**, not snake_case_names, see **https://example.com/a_b**
        - tabs
        	- nested once
        		- nested twice
    """.trimIndent()

    @Test fun markdownTablesAndLists() = shoot("sweep-markdown") {
        Box(Modifier.padding(16.dp)) { MarkdownText(markdown) }
    }

    @Test fun viewerWithAnImageThatWillNotLoad() = shoot("sweep-viewer-failed") {
        Box(Modifier.fillMaxSize().background(Color.Black)) {
            // Nobody is signed in here: the image can't be had, which is the state to look at.
            ViewerImage(Media("{\"url\":\"mxc://example.org/none\"}", null, 800, 600), "Image")
            ViewerBar({}) {
                ViewerButton(Icons.Outlined.Download, "Save the image") {}
                ViewerButton(Icons.Outlined.Share, "Share the image") {}
            }
        }
    }

    private fun sha(s: String) =
        MessageDigest.getInstance("SHA-256").digest(s.toByteArray()).take(12).joinToString("") { "%02x".format(it) }

    @Test
    fun aSaveStillLandsWhenItsCardHasLeftTheScreen() {
        val app = RuntimeEnvironment.getApplication()
        val f = FileRef("{\"url\":\"mxc://example.org/report\"}", "report.pdf", "application/pdf", 5, FileKind.File)
        // Already downloaded (where Files.fetch keeps it): the save needs no server.
        File(app.cacheDir, "files/${sha(f.source)}/report.pdf").apply { parentFile!!.mkdirs() }.writeText("hello")
        var onScreen by mutableStateOf(true)
        var activity: Activity? = null
        show {
            activity = LocalContext.current as Activity
            Column { if (onScreen) FileCard(f) else Box(Modifier.fillMaxWidth().height(48.dp)) }
        }
        compose.onNodeWithContentDescription("Save report.pdf").performClick()
        val shadow = shadowOf(activity!!)
        val asked = shadow.nextStartedActivityForResult
        assertNotNull("the save picker opens", asked)
        assertEquals(Intent.ACTION_CREATE_DOCUMENT, asked.intent.action)
        assertEquals("report.pdf", asked.intent.getStringExtra(Intent.EXTRA_TITLE))
        assertEquals("application/pdf", asked.intent.type)

        // New messages push the card out of the list while the picker is open:
        // with the launcher registered by the card, its answer went nowhere.
        onScreen = false
        compose.waitForIdle()
        val target = Uri.parse("content://documents/report.pdf")
        val written = ByteArrayOutputStream()
        shadowOf(app.contentResolver).registerOutputStream(target, written)
        shadow.receiveResult(asked.intent, Activity.RESULT_OK, Intent().setData(target))

        // The copy runs off the main thread and reports back on it.
        val deadline = System.currentTimeMillis() + 10_000
        while ((written.size() == 0 || FileJobs.busy.value.isNotEmpty()) && System.currentTimeMillis() < deadline) {
            shadowOf(Looper.getMainLooper()).idle()
            Thread.sleep(20)
        }
        assertEquals("hello", written.toString())
        assertEquals("Saved report.pdf", ShadowToast.getTextOfLatestToast())
    }
}
