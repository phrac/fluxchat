package org.fluxchat.companion.ui

import androidx.activity.ComponentActivity
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import org.junit.After
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * "Send to…" ends the share when you leave it without picking a project —
 * but being rebuilt under you (the phone was turned) is not leaving.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35])
class ShareRebuiltTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()

    @Before
    fun setUp() {
        ShareInbox.pending.value = null
        ShareInbox.target.value = null
    }

    @After
    fun tearDown() {
        ShareInbox.pending.value = null
        ShareInbox.target.value = null
    }

    @Test
    fun turningThePhoneAtSendToKeepsWhatWasShared() {
        ShareInbox.offer(Shared("look at this", null))
        var takenDown = false
        compose.setContent {
            DisposableEffect(Unit) { onDispose { takenDown = true } }
            FluxTheme { ShareScreen(back = {}, swap = {}) }
        }
        compose.onNodeWithText("Send to…").assertIsDisplayed()
        compose.activityRule.scenario.recreate()
        assertTrue("the screen went with its activity", takenDown)
        // It used to end the share on its way out, and the stack came back without it.
        assertNotNull(ShareInbox.pending.value)
        assertNotNull(decodeScreen(encodeScreen(Screen.Share)!!))
    }

    @Test
    fun leavingSendToWithoutAProjectEndsTheShare() {
        ShareInbox.offer(Shared("look at this", null))
        var shown by mutableStateOf(true)
        compose.setContent { FluxTheme { if (shown) ShareScreen(back = {}, swap = {}) } }
        compose.onNodeWithText("Send to…").assertIsDisplayed()
        shown = false
        compose.waitForIdle()
        assertNull(ShareInbox.pending.value)
    }
}
