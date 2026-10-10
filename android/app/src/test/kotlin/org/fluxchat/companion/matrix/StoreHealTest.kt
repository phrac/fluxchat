package org.fluxchat.companion.matrix

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class StoreHealTest {
    @Test fun `an unacknowledged native crash at the first chance heals`() {
        assertTrue(StoreHeal.decide(unackedNativeCrash = true, heals = 0))
    }

    @Test fun `a clean last death never rebuilds the store`() {
        assertFalse(StoreHeal.decide(unackedNativeCrash = false, heals = 0))
        assertFalse(StoreHeal.decide(unackedNativeCrash = false, heals = 1))
    }

    @Test fun `the ladder stops after two heals`() {
        assertTrue(StoreHeal.decide(unackedNativeCrash = true, heals = 1))
        assertFalse(StoreHeal.decide(unackedNativeCrash = true, heals = 2))
        assertFalse(StoreHeal.decide(unackedNativeCrash = true, heals = 9))
    }

    @Test fun `a clean process between crashes re-arms one heal`() {
        // A death that wasn't a native crash resets heals to 0 (bootStarted's
        // job); from 0 the policy heals again.
        assertTrue(StoreHeal.decide(unackedNativeCrash = true, heals = 0))
    }
}
