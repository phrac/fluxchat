package org.fluxchat.companion.push

import org.fluxchat.companion.push.Push.Plan
import org.fluxchat.companion.push.Push.Saved
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * "You want push" and "this phone is registered for push" are two things.
 * Signing out ended both: after signing back in nothing arrived until you
 * found the switch in Settings and turned it on again.
 */
class PushPlanTest {
    private val ntfy = "io.heckel.ntfy"
    private val registered = Saved(
        enabled = true, distributor = ntfy, endpoint = "https://ntfy.sh/upTOPIC",
        pushedEndpoint = "https://ntfy.sh/upTOPIC", gateway = "https://ntfy.sh/_matrix/push/v1/notify",
    )

    @Test fun `signing out ends the registration and keeps the choice`() {
        val after = registered.signedOut()
        assertTrue(after.enabled)
        assertEquals(ntfy, after.distributor)
        // The endpoint was this account's: the next one gets a new one.
        assertNull(after.endpoint)
        assertNull(after.pushedEndpoint)
        assertNull(after.gateway)
    }

    @Test fun `turning push off is remembered as off`() {
        val after = registered.turnedOff()
        assertFalse(after.enabled)
        assertNull(after.endpoint)
        assertNull(after.pushedEndpoint)
    }

    @Test fun `the next sign-in registers again with the distributor you chose`() {
        // Unregistering made the connector forget its distributor: nothing is acknowledged.
        val plan = Push.plan(registered.signedOut(), acked = null, installed = listOf(ntfy, "org.other.distributor"))
        assertEquals(Plan.Reconnect(ntfy), plan)
    }

    @Test fun `push you turned off stays off across sign-ins`() {
        assertEquals(Plan.Off, Push.plan(registered.turnedOff(), acked = null, installed = listOf(ntfy)))
        assertEquals(Plan.Off, Push.plan(Saved(), acked = ntfy, installed = listOf(ntfy)))
    }

    @Test fun `a live registration is renewed, not started over`() {
        assertEquals(Plan.Renew(ntfy), Push.plan(registered, acked = ntfy, installed = listOf(ntfy)))
    }

    @Test fun `the distributor you chose is gone`() {
        val signedOut = registered.signedOut()
        // Nothing to push through.
        assertEquals(Plan.NoDistributor, Push.plan(signedOut, acked = null, installed = emptyList()))
        // One other: that's the one.
        assertEquals(Plan.Reconnect("org.other"), Push.plan(signedOut, acked = null, installed = listOf("org.other")))
        // Several: not ours to choose between.
        assertEquals(Plan.Pick, Push.plan(signedOut, acked = null, installed = listOf("org.other", "org.third")))
    }
}
