package org.fluxchat.companion.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import org.fluxchat.companion.matrix.AgentEntry
import org.fluxchat.companion.matrix.AgentList
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * The Projects-screen creation flow: the dialog's copy and what submitting
 * it does (calls the project creation helper, then navigates into the new
 * project). The Matrix call is injected so the test watches the contract,
 * not the network.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35], qualifiers = "w411dp-h891dp-xxhdpi")
class ProjectCreateFlowTest {
    @get:Rule val compose = createComposeRule()

    private val agents = AgentList(agents = listOf(AgentEntry("@hermes:example.org"), AgentEntry("@ricky:example.org")))

    private fun flow(
        list: AgentList = agents,
        create: suspend (String, AgentList) -> String?,
    ): List<Screen> {
        val navigated = ArrayList<Screen>()
        compose.setContent {
            FluxTheme {
                Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) {
                    CreateProjectFlow(list = list, go = navigated::add, onCreated = {}, onDismiss = {}, create = create)
                }
            }
        }
        return navigated
    }

    @Test fun dialogAsksForAProjectName() {
        flow { n, _ -> n; "!created:example.org" }
        compose.onNodeWithText("New project").assertIsDisplayed()
        compose.onNodeWithText("Project name").assertIsDisplayed()
        // The invite promise matches the agent list.
        compose.onNodeWithText("Your 2 agents are invited and can start right away.").assertIsDisplayed()
    }

    @Test fun noAgentsTellsYouToAddOne() {
        flow(list = AgentList()) { _, _ -> "!created:example.org" }
        compose.onNodeWithText("Add agents first and they'll be invited to every new project.").assertIsDisplayed()
    }

    @Test fun createIsDisabledUntilTheNameIsFilledIn() {
        flow { _, _ -> "!created:example.org" }
        compose.onNodeWithText("Create").assertIsNotEnabled()
        compose.onNodeWithText("Project name").performTextInput("Weekend build")
        compose.onNodeWithText("Create").assertIsEnabled()
    }

    @Test fun submittingTheNameCreatesTheProjectAndNavigatesIntoIt() {
        var calledName: String? = null
        var calledList: AgentList? = null
        val navigated = flow { name, list ->
            calledName = name
            calledList = list
            "!fresh:example.org"
        }
        compose.onNodeWithText("Project name").performTextInput("Weekend build")
        compose.onNodeWithText("Create").performClick()
        compose.waitForIdle()
        assertEquals("Weekend build", calledName)
        assertEquals(agents, calledList)
        assertEquals(listOf<Screen>(Screen.Room("!fresh:example.org")), navigated)
    }

    @Test fun aFailedCreationStaysOnTheDialog() {
        val navigated = flow { _, _ -> null }
        compose.onNodeWithText("Project name").performTextInput("Broken")
        compose.onNodeWithText("Create").performClick()
        compose.waitForIdle()
        assertTrue("a failed create must not navigate", navigated.isEmpty())
        compose.onNodeWithText("New project").assertIsDisplayed()
    }

    @Test fun dialogCopySpeaksProjectsNotTheProtocol() {
        flow { _, _ -> null }
        // The visible copy: "project" yes, "room"/"channel" never.
        for (s in listOf("Project name", "Create", "Cancel")) compose.onNodeWithText(s).assertIsDisplayed()
        assertTrue("a room string leaked", compose.onAllNodes(matcher = androidx.compose.ui.test.hasText("room", ignoreCase = true, substring = true)).fetchSemanticsNodes().isEmpty())
        assertTrue("a channel string leaked", compose.onAllNodes(matcher = androidx.compose.ui.test.hasText("channel", ignoreCase = true, substring = true)).fetchSemanticsNodes().isEmpty())
    }
}
