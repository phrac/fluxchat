package org.fluxchat.companion.ui

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import org.junit.Assert.assertEquals
import org.junit.Test

/** The image viewers' panning, and which run-history rows still have somewhere to go. */
class ViewerTest {
    private val phone = Size(1080f, 2400f)

    @Test
    fun anImageIsFittedInsideTheScreen() {
        assertEquals("a wide image fills the width", Size(1080f, 540f), fittedSize(Size(4000f, 2000f), phone))
        assertEquals("a tall one the height", Size(600f, 2400f), fittedSize(Size(1000f, 4000f), phone))
        assertEquals("nothing to fit before the screen is measured", Size.Zero, fittedSize(Size(4000f, 2000f), Size.Zero))
    }

    @Test
    fun anImageThatFitsCannotBeDragged() {
        assertEquals(Offset.Zero, clampPan(Offset(300f, -900f), 1f, Size(4000f, 2000f), phone))
    }

    @Test
    fun aZoomedImageStopsAtItsOwnEdges() {
        // 1080 × 540 on screen, three times over: 3240 × 1620. It sticks out
        // 2160 sideways (so 1080 either way) and not at all up or down.
        val wide = Size(4000f, 2000f)
        assertEquals(Offset(1080f, 0f), clampPan(Offset(5000f, 700f), 3f, wide, phone))
        assertEquals(Offset(-1080f, 0f), clampPan(Offset(-5000f, -700f), 3f, wide, phone))
        assertEquals("within the edges it goes where the finger does", Offset(-200f, 0f), clampPan(Offset(-200f, 0f), 3f, wide, phone))
        // Six times over it is 3240 tall: 420 of play up and down too.
        assertEquals(Offset(100f, 420f), clampPan(Offset(100f, 9000f), 6f, wide, phone))
    }

    @Test
    fun aRunsRowIsClosedOnlyWhenItsProjectIsGone() {
        val recorded = setOf("!live", "!joined-no-agent", "!left")
        val gone = goneProjects(recorded, live = setOf("!live")) { it == "!joined-no-agent" }
        // Still one of your agents' projects, or at least one you are in: it opens.
        assertEquals(setOf("!left"), gone)
    }
}
