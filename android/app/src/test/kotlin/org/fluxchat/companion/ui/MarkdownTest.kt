package org.fluxchat.companion.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.font.FontStyle
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class MarkdownTest {
    private val colors = InlineColors(Color.Red, TextLinkStyles())

    private fun links(text: String) = inline(text, colors).let { a ->
        a.getLinkAnnotations(0, a.length).map { (it.item as LinkAnnotation.Url).url }
    }

    @Test
    fun aCodeBlockUnderAListItemIsACodeBlock() {
        val blocks = parseBlocks("1. Install it:\n   ```bash\n   make install\n   ```\n2. Run it")
        assertEquals(3, blocks.size)
        assertEquals("Install it:", (blocks[0] as Block.ListBlock).items.single().text)
        val code = blocks[1] as Block.Code
        assertEquals("bash", code.lang)
        assertEquals("the item's indent comes off", "make install", code.text)
        assertEquals("2.", (blocks[2] as Block.ListBlock).items.single().marker)
    }

    @Test
    fun yamlInAFenceStaysCode() {
        val blocks = parseBlocks("- Config:\n  ```yaml\n  - name: x\n    run: y\n  ```")
        assertEquals("- name: x\n  run: y", (blocks[1] as Block.Code).text)
    }

    @Test
    fun aOneLineFenceDoesNotSwallowTheRest() {
        val blocks = parseBlocks("Run:\n```make test```\nThen read the log.")
        assertEquals("make test", (blocks[1] as Block.Code).text)
        assertEquals("Then read the log.", (blocks[2] as Block.Paragraph).text)
    }

    @Test
    fun onlyABareFenceAtLeastAsLongCloses() {
        val blocks = parseBlocks("````md\n```python\nprint(1)\n```\n````\nafter")
        assertEquals("```python\nprint(1)\n```", (blocks[0] as Block.Code).text)
        assertEquals("after", (blocks[1] as Block.Paragraph).text)
    }

    @Test
    fun globsAreNotItalics() {
        assertEquals("Matches *.kt and *.kts files", inline("Matches *.kt and *.kts files", colors).text)
        val a = inline("this is *really* it", colors)
        assertEquals("this is really it", a.text)
        assertTrue(a.spanStyles.any { it.item.fontStyle == FontStyle.Italic })
    }

    @Test
    fun linksKeepTheirParentheses() {
        assertEquals(
            listOf("https://en.wikipedia.org/wiki/Foo_(bar)"),
            links("[Foo](https://en.wikipedia.org/wiki/Foo_(bar))"),
        )
        assertEquals("Foo", inline("[Foo](https://en.wikipedia.org/wiki/Foo_(bar))", colors).text)
        assertEquals(listOf("https://en.wikipedia.org/wiki/Foo_(bar)"), links("see https://en.wikipedia.org/wiki/Foo_(bar)."))
        // A URL in brackets still ends before the bracket, and before the full stop.
        assertEquals(listOf("https://example.com/a"), links("(see https://example.com/a)."))
    }
}
