package org.fluxchat.companion.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.isSpecified
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

    private fun AnnotatedString.styled(what: (SpanStyle) -> Boolean) =
        spanStyles.filter { what(it.item) }.map { text.substring(it.start, it.end) }
    private fun AnnotatedString.bold() = styled { it.fontWeight == FontWeight.SemiBold }
    private fun AnnotatedString.italic() = styled { it.fontStyle == FontStyle.Italic }
    private fun AnnotatedString.code() = styled { it.fontFamily == FontFamily.Monospace }
    private fun AnnotatedString.linked() =
        getLinkAnnotations(0, length).map { text.substring(it.start, it.end) to (it.item as LinkAnnotation.Url).url }

    @Test
    fun aPipeInCodeOrEscapedStaysInItsCell() {
        // split('|') made five cells of these three, and "z" landed under the wrong heading.
        assertEquals(listOf("`a | b`", "x | y", "z"), tableCells("| `a | b` | x \\| y | z |"))
        assertEquals(listOf("a", "", "c"), tableCells("| a || c |"))
        assertEquals("no closing pipe", listOf("a", "b"), tableCells("| a | b"))
        // A backtick with no partner protects nothing.
        assertEquals(listOf("it`s", "b"), tableCells("it`s | b"))
    }

    @Test
    fun aTableCellShowsItsLinkTextAndKeepsTheLink() {
        val table = parseBlocks("| Name | Where |\n|------|:-----:|\n| **foo** | [docs](https://example.com/d) |\n| `a|b` | n/a |").single() as Block.Table
        assertTrue(table.header)
        assertEquals(3, table.rows.size)
        val text = tableText(table, colors)
        // Columns are as wide as what shows, not as the markup that made it.
        assertEquals("Name  Where\nfoo   docs\na|b   n/a", text.text)
        assertEquals(listOf("docs" to "https://example.com/d"), text.linked())
        assertEquals(listOf("Name  Where", "foo"), text.bold())
        // Code in a table keeps the text's size: a smaller cell would break the columns.
        assertTrue(text.spanStyles.none { it.item.fontSize.isSpecified })
    }

    @Test
    fun aTableWithoutLeadingPipesIsATable() {
        val blocks = parseBlocks("Results:\nName | Value\n--- | ---\na | 1\nb | 2\n\nafter")
        assertEquals("Results:", (blocks[0] as Block.Paragraph).text)
        val table = blocks[1] as Block.Table
        assertTrue(table.header)
        assertEquals(listOf(listOf("Name", "Value"), listOf("a", "1"), listOf("b", "2")), table.rows)
        assertEquals("after", (blocks[2] as Block.Paragraph).text)
    }

    @Test
    fun aLineWithAPipeAloneIsNotATable() {
        assertEquals("grep foo | wc -l\nprints 3", (parseBlocks("grep foo | wc -l\nprints 3").single() as Block.Paragraph).text)
        // A rule under it is a rule: the separator must have as many columns as the header.
        val blocks = parseBlocks("a | b\n---")
        assertEquals("a | b", (blocks[0] as Block.Paragraph).text)
        assertEquals(Block.Rule, blocks[1])
    }

    @Test
    fun aRowOfSingleDashesUnderTheHeaderSeparatesAndFurtherDownIsData() {
        val table = parseBlocks("| a | b |\n|-|-|\n| 1 | 2 |\n| - | - |").single() as Block.Table
        assertTrue(table.header)
        assertEquals(listOf(listOf("a", "b"), listOf("1", "2"), listOf("-", "-")), table.rows)
    }

    private fun depths(body: String) = (parseBlocks(body).single() as Block.ListBlock).items.map { it.depth }

    @Test
    fun aListNestsOneLevelPerIndentWhateverItsWidth() {
        assertEquals("two spaces", listOf(0, 1, 2, 0), depths("- a\n  - b\n    - c\n- d"))
        assertEquals("four spaces were two levels each", listOf(0, 1, 2, 1, 0), depths("- a\n    - b\n        - c\n    - d\n- e"))
        assertEquals("tabs were flat", listOf(0, 1, 2), depths("- a\n\t- b\n\t\t- c"))
        assertEquals("under a number", listOf(0, 1, 1, 0), depths("1. a\n   - b\n   - c\n2. d"))
        assertEquals("a list that starts indented starts at the margin", listOf(0, 0, 1), depths("  - a\n  - b\n    - c"))
        assertEquals("back out to an indent between two levels", listOf(0, 1, 1, 0), listDepths(listOf(0, 4, 2, 0)))
        assertEquals(4, indentWidth("\t"))
        assertEquals(4, indentWidth("  \t"))
        assertEquals(8, indentWidth("\t\t"))
    }

    @Test
    fun stylesNestTheWayTheyAreWritten() {
        val fixed = inline("**Fixed `foo()`** today", colors)
        assertEquals("Fixed foo() today", fixed.text)
        assertEquals(listOf("Fixed foo()"), fixed.bold())
        assertEquals(listOf("foo()"), fixed.code())

        val pr = inline("**[PR 12](https://example.com/pr/12)**", colors)
        assertEquals("PR 12", pr.text)
        assertEquals(listOf("PR 12" to "https://example.com/pr/12"), pr.linked())
        assertEquals(listOf("PR 12"), pr.bold())

        val see = inline("**see https://example.com/a_b**", colors)
        assertEquals("see https://example.com/a_b", see.text)
        assertEquals(listOf("https://example.com/a_b" to "https://example.com/a_b"), see.linked())

        val both = inline("***both*** and *one **two***", colors)
        assertEquals("both and one two", both.text)
        assertEquals(listOf("both", "two"), both.bold())
        assertEquals(listOf("both", "one two"), both.italic())

        val label = inline("[the `x` **flag**](https://example.com)", colors)
        assertEquals("the x flag", label.text)
        assertEquals(listOf("x"), label.code())
        assertEquals(listOf("flag"), label.bold())

        assertEquals("gone, ~/.cache, ~5 min", inline("~~gone~~, ~/.cache, ~5 min", colors).text)
        assertEquals(listOf("https://example.com/a"), links("<https://example.com/a>"))
        assertEquals("https://example.com/a", inline("<https://example.com/a>", colors).text)
    }

    @Test
    fun nothingInsideCodeIsMarkup() {
        val a = inline("run `rm *.tmp **now**` or `[x](https://example.com)`", colors)
        assertEquals("run rm *.tmp **now** or [x](https://example.com)", a.text)
        assertTrue(a.bold().isEmpty())
        assertTrue(a.linked().isEmpty())
        // The star before the code and the one inside it are not a pair.
        val b = inline("a *b `c*` d", colors)
        assertEquals("a *b c* d", b.text)
        assertTrue(b.italic().isEmpty())
    }

    @Test
    fun underscoresItaliciseWordsNotSnakeCase() {
        val a = inline("it is _really_ so", colors)
        assertEquals("it is really so", a.text)
        assertEquals(listOf("really"), a.italic())
        for (plain in listOf("snake_case_names and other_thing", "a_b_c", "set MAX_SIZE_ then _MIN", "2*3*4 and 2 * 3 * 4", "foo__bar__baz")) {
            val b = inline(plain, colors)
            assertEquals(plain, b.text)
            assertTrue(plain, b.spanStyles.isEmpty())
        }
        assertEquals(listOf("init"), inline("__init__ runs first", colors).bold())
    }

    @Test
    fun aBackslashMakesMarkupLiteral() {
        val a = inline("\\*not italic\\* and \\[not\\](a link) and 1\\. item", colors)
        assertEquals("*not italic* and [not](a link) and 1. item", a.text)
        assertTrue(a.spanStyles.isEmpty())
        // Not an escape: a path keeps its backslashes.
        assertEquals("C:\\Users\\derek\\notes", inline("C:\\Users\\derek\\notes", colors).text)
        assertEquals("an escaped star does not close", "*a* b*", inline("\\*a\\* b*", colors).text)
    }

    @Test
    fun whatIsNotALinkShowsAsWritten() {
        for (plain in listOf("[1] and [2]", "- [ ] todo [x] done", "[file](src/main.rs)", "[a](https://example.com not a title)", "f(x) = [a, b](c)", "[](https://example.com)x")) {
            val a = inline(plain, colors)
            assertTrue(plain, a.text.startsWith(plain.substringBefore("https://")))
            assertTrue(plain, a.linked().none { it.first != it.second })
        }
        assertEquals(listOf("docs" to "https://example.com/d"), inline("[docs](https://example.com/d \"The docs\")", colors).linked())
    }

    @Test
    fun aHugeLineOfMarkupCharactersReturnsPromptly() {
        // Agents paste minified output: one line, 100 KB. The regexes this
        // parser replaced were quadratic on some of these (each "[" looked
        // for its "]" to the end of the line, and again from the next "["):
        // 4 s for 20 KB of "[", 0.4 s for 20 KB of "*a* ".
        val units = listOf(
            "*", "_", "~", "`", "[", "]", "(", ")", "\\", "|", "<", "h",
            "*a ", "_a ", "**a ", "~~a ", "*a _b ", "_a *b_ ", "`a ``b ", "[a](", "[a](http://x ", "[a](http://x",
            "https://(", "https://a(b ", "<https://", "a|`b", "*a* ", "**a** [b](https://c.d) `e` ",
        )
        val lines = units.map { it to it.repeat(100_000 / it.length + 1) } + listOf(
            // Every opener has its closer, far away: nothing gives up early.
            "[…]" to "[".repeat(50_000) + "]".repeat(50_000),
            "(…)" to "(".repeat(50_000) + ")".repeat(50_000),
            "[a](http://x …)" to "[a](http://x ".repeat(8_000) + ")".repeat(8_000),
            "https://(…)" to "https://(".repeat(10_000) + " " + ")".repeat(10_000),
            "*a _b … b_ a*" to "*a _b ".repeat(8_000) + "b_ a* ".repeat(8_000),
            "*a **b … b** a*" to "*a **b ".repeat(8_000) + "b** a* ".repeat(8_000),
        )
        for ((unit, line) in lines) {
            val started = System.nanoTime()
            val blocks = parseBlocks("x " + line)
            for (b in blocks) when (b) {
                is Block.Paragraph -> inline(b.text, colors)
                is Block.Table -> tableText(b, colors)
                else -> {}
            }
            inline(line, colors)
            tableCells(line)
            val ms = (System.nanoTime() - started) / 1_000_000
            assertTrue("100 KB of \"$unit\" took $ms ms", ms < 2_000)
        }
    }
}
