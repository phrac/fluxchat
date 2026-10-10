package org.fluxchat.companion

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class UpdateTest {
    @Test
    fun newerReleasesWin() {
        assertTrue(Update.isNewer("0.2.1", "0.2.2"))
        assertTrue(Update.isNewer("0.2.1", "0.3"))      // more parts, not bigger digits
        assertTrue(Update.isNewer("0.9.0", "0.10.0"))   // numeric, not lexicographic
        assertTrue(Update.isNewer("0.2.2", "0.2.2.1"))  // longer wins ties
    }

    @Test
    fun equalAndOlderLose() {
        assertFalse(Update.isNewer("0.2.1", "0.2.1"))
        assertFalse(Update.isNewer("0.2.2", "0.2.1"))
        assertFalse(Update.isNewer("0.2.2.1", "0.2.2"))
        assertFalse(Update.isNewer("0.2.1", "0.2"))
    }
}
