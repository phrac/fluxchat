package org.fluxchat.companion.push

import android.app.NotificationManager
import android.content.Context
import android.content.pm.ServiceInfo
import androidx.test.core.app.ApplicationProvider
import org.fluxchat.companion.agents.RunInfo
import org.fluxchat.companion.agents.RunState
import org.fluxchat.companion.agents.ViewKey
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

/** The live runs, as the home-screen widget's rows and the ongoing notification have them. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35]) // the newer sandbox image breaks on the build host's JDK
class LiveRunsTest {
    private val context: Context by lazy { ApplicationProvider.getApplicationContext() }

    private fun live(state: RunState, step: String? = null) = LiveRuns.Live(
        ViewKey("!ops:example.org", "\$request"), RunInfo(state, since = 1_000, step = step, steps = if (step == null) 0 else 3),
        agent = "@hermes:example.org", title = "fix the welder fault", room = "maintdx", name = "Hermes",
    )

    @Test fun `a widget row is headed by the agent's name, not their address`() {
        // "@hermes:example.org · fix the welder fault", in a row one line
        // high: the address took the room the session's title needed.
        val row = LiveRuns.widgetRow(live(RunState.Working, "🔧 Running tests"), "!stop")
        assertEquals("Hermes · fix the welder fault", row.title)
        assertEquals("🔧 Running tests", row.sub)
        // Where a tap goes, and what Stop sends, are still the run's own.
        assertEquals("!ops:example.org", row.roomId)
        assertEquals("\$request", row.threadRoot)
        assertEquals("!stop", row.stop)
    }

    @Test fun `an agent nobody has named yet goes by the first part of the address`() {
        val l = LiveRuns.Live(ViewKey("!ops:example.org"), RunInfo(RunState.Working, since = 1_000), "@hermes:example.org", "maintdx", "maintdx")
        val row = LiveRuns.widgetRow(l, null)
        assertEquals("hermes · maintdx", row.title)
        assertEquals("Working", row.sub)
        assertNull(row.stop)
    }

    @Test fun `a run that waits on you says so in its row`() {
        // It kept the last tool step it ran before asking: a row that read as still working.
        assertEquals("Needs you", LiveRuns.widgetRow(live(RunState.NeedsYou, "🔧 Running tests"), "!stop").sub)
        assertEquals("Needs you", LiveRuns.widgetRow(live(RunState.NeedsYou), "!stop").sub)
    }

    @Test fun `the service timing out leaves the notification up`() {
        // Android 15's six hours a day of data-sync service are spent while
        // the runs go on: the notification went with the service, and came
        // back only at a run's next step.
        val service = Robolectric.buildService(RunService::class.java).create().get()
        service.startForeground(LiveRuns.NOTIFICATION_ID, LiveRuns.idle(context))
        service.onTimeout(1, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        val shadow = shadowOf(service)
        assertTrue(shadow.isStoppedBySelf)
        assertTrue(shadow.isForegroundStopped)
        val shown = shadowOf(context.getSystemService(NotificationManager::class.java)).getNotification(LiveRuns.NOTIFICATION_ID)
        assertTrue(shown != null)
    }
}
