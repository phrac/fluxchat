package org.fluxchat.companion.quick

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.fluxchat.companion.quick.RunWidgetData
import org.fluxchat.companion.quick.RunWidgetRow
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class RunWidgetTest {
    private val context: Context by lazy { ApplicationProvider.getApplicationContext() }

    @Before
    fun reset() = RunWidgetData.clear(context)

    private fun row(i: Int, stop: String? = "!stop") =
        RunWidgetRow("k$i", "!room$i:y", if (i % 2 == 0) "\$root$i" else null, "title $i", "step $i", stop)

    @Test fun `an empty snapshot loads as no rows`() {
        assertEquals(0, RunWidgetData.load(context).size)
    }

    @Test fun `rows round-trip including missing thread and stop`() {
        val rows = listOf(row(0), row(1, stop = null))
        RunWidgetData.save(context, rows)
        val back = RunWidgetData.load(context)
        assertEquals(2, back.size)
        assertEquals("\$root0", back[0].threadRoot)
        assertEquals("!stop", back[0].stop)
        assertNull(back[1].threadRoot)
        assertNull(back[1].stop)
    }

    @Test fun `titles survive separators and unicode`() {
        val r = RunWidgetRow("k", "!r:y", null, "ship |it|\nnow — ✓ 🚀", "s", null)
        RunWidgetData.save(context, listOf(r))
        assertEquals(r.title, RunWidgetData.load(context).single().title)
    }

    @Test fun `the snapshot caps at the widget's row limit`() {
        RunWidgetData.save(context, (0..9).map { row(it) })
        assertEquals(RunWidgetData.MAX_ROWS, RunWidgetData.load(context).size)
    }

    @Test fun `a corrupted snapshot loads as no rows, not a crash`() {
        context.getSharedPreferences("runwidget", Context.MODE_PRIVATE).edit().putString("rows", "{{{").commit()
        assertEquals(0, RunWidgetData.load(context).size)
    }
}
