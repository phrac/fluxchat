package org.fluxchat.companion.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Check
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay

/** Size of code (blocks and tables). */
private val CODE_SIZE = 12.5.sp
private val CODE_LINE_HEIGHT = 18.sp

/**
 * A message body: Markdown as agents write it — headings, lists (nested,
 * numbered) with hanging indents, quotes, rules, tables, fenced code with a
 * copy button, and inline styles and links.
 */
@Composable
fun MarkdownText(
    body: String,
    color: Color = MaterialTheme.colorScheme.onSurface,
    maxLines: Int = Int.MAX_VALUE,
    style: TextStyle = MaterialTheme.typography.bodyMedium,
) {
    val blocks = remember(body) { parseBlocks(body) }
    val scheme = MaterialTheme.colorScheme
    val colors = InlineColors(scheme.tertiary, TextLinkStyles(SpanStyle(color = scheme.primary, textDecoration = TextDecoration.Underline)))
    SelectionContainer {
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            var linesLeft = maxLines
            for (b in blocks) {
                if (linesLeft <= 0) break
                when (b) {
                    is Block.Paragraph -> {
                        val text = remember(b.text, colors) { inline(b.text, colors) }
                        Text(text, color = color, style = style, maxLines = linesLeft)
                    }
                    is Block.Heading -> {
                        val text = remember(b.text, colors) { inline(b.text, colors) }
                        Text(
                            text, color = color, maxLines = linesLeft,
                            style = when (b.level) {
                                1 -> MaterialTheme.typography.titleLarge
                                2 -> MaterialTheme.typography.titleMedium
                                else -> MaterialTheme.typography.titleSmall
                            }.copy(fontWeight = FontWeight.SemiBold),
                            modifier = Modifier.padding(top = if (b.level <= 2) 6.dp else 2.dp),
                        )
                    }
                    is Block.ListBlock -> Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        for (item in b.items) {
                            Row(Modifier.padding(start = (item.depth * 18).dp)) {
                                Text(
                                    item.marker, style = style, color = scheme.onSurfaceVariant,
                                    modifier = Modifier.widthIn(min = if (item.marker.length > 2) 26.dp else 18.dp),
                                )
                                val text = remember(item.text, colors) { inline(item.text, colors) }
                                Text(text, color = color, style = style, modifier = Modifier.weight(1f))
                            }
                        }
                    }
                    is Block.Quote -> Row(Modifier.height(IntrinsicSize.Min)) {
                        Box(Modifier.width(3.dp).fillMaxHeight().background(scheme.outlineVariant, RoundedCornerShape(2.dp)))
                        Spacer(Modifier.width(10.dp))
                        val text = remember(b.text, colors) { inline(b.text, colors) }
                        Text(text, color = scheme.onSurfaceVariant, style = style.copy(fontStyle = FontStyle.Italic))
                    }
                    Block.Rule -> HorizontalDivider(color = scheme.outlineVariant)
                    is Block.Code -> CodeBlock(AnnotatedString(b.text), b.lang, maxLines = linesLeft)
                    is Block.Table -> {
                        val text = remember(b, colors) { tableText(b, colors) }
                        CodeBlock(text, null, maxLines = linesLeft, copy = false)
                    }
                }
                linesLeft -= b.lines
            }
        }
    }
}

/** Fenced code: its language and a copy button over a scrollable monospace body. */
@Composable
private fun CodeBlock(code: AnnotatedString, lang: String?, maxLines: Int, copy: Boolean = true) {
    val scheme = MaterialTheme.colorScheme
    val clipboard = LocalClipboardManager.current
    var copied by remember { mutableStateOf(false) }
    LaunchedEffect(copied) { if (copied) { delay(1500); copied = false } }
    Surface(color = scheme.surfaceContainerHigh, shape = RoundedCornerShape(12.dp), modifier = Modifier.fillMaxWidth()) {
        Column {
            if (copy) {
                Row(Modifier.fillMaxWidth().padding(start = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(lang ?: "code", style = MaterialTheme.typography.labelSmall, color = scheme.onSurfaceVariant, modifier = Modifier.weight(1f))
                    // No size of its own: at 36 dp it was a smaller target than a finger wants (48 dp).
                    IconButton(onClick = { clipboard.setText(AnnotatedString(code.text)); copied = true }) {
                        Icon(if (copied) Icons.Outlined.Check else Icons.Outlined.ContentCopy, if (copied) "Copied" else "Copy code", Modifier.size(16.dp), tint = scheme.onSurfaceVariant)
                    }
                }
            }
            Text(
                code,
                fontFamily = FontFamily.Monospace,
                fontSize = CODE_SIZE,
                lineHeight = CODE_LINE_HEIGHT,
                color = scheme.onSurface,
                softWrap = false,
                maxLines = maxLines,
                modifier = Modifier.fillMaxWidth().horizontalScroll(rememberScrollState())
                    .padding(start = 12.dp, end = 12.dp, top = if (copy) 0.dp else 10.dp, bottom = 10.dp),
            )
        }
    }
}

internal sealed interface Block {
    /** Lines it takes, for folding long answers. */
    val lines: Int

    data class Paragraph(val text: String) : Block { override val lines get() = text.lines().size }
    data class Heading(val level: Int, val text: String) : Block { override val lines get() = 1 }
    data class Item(val marker: String, val depth: Int, val text: String)
    data class ListBlock(val items: List<Item>) : Block { override val lines get() = items.sumOf { it.text.lines().size } }
    data class Quote(val text: String) : Block { override val lines get() = text.lines().size }
    data object Rule : Block { override val lines get() = 1 }
    data class Code(val text: String, val lang: String?) : Block { override val lines get() = text.lines().size + 1 }
    /** Rows of cells as written (inline Markdown inside); [header]: the first row is one (a separator row followed it). */
    data class Table(val rows: List<List<String>>, val header: Boolean = false) : Block { override val lines get() = rows.size }
}

private val HEADING = Regex("^(#{1,6})\\s+(.*)$")
private val ITEM = Regex("^(\\s*)([-*+•]|\\d{1,3}[.)])\\s+(.*)$")
private val RULE = Regex("^(-{3,}|\\*{3,}|_{3,})$")

/** Markdown blocks, line by line (agents write simple Markdown; nothing exotic). */
internal fun parseBlocks(body: String): List<Block> {
    val out = ArrayList<Block>()
    val lines = body.lines()
    val para = ArrayList<String>()
    fun flush() {
        if (para.isNotEmpty()) out += Block.Paragraph(para.joinToString("\n"))
        para.clear()
    }
    var i = 0
    while (i < lines.size) {
        val raw = lines[i]
        val line = raw.trim()
        when {
            line.startsWith("```") -> {
                flush()
                val ticks = line.takeWhile { it == '`' }.length
                val rest = line.drop(ticks).trim()
                if (rest.endsWith("```") && rest.trim('`').isNotBlank()) {
                    // ```cmd``` on one line is the whole block (it used to
                    // open a fence that swallowed the rest of the message).
                    out += Block.Code(rest.trimEnd('`').trim(), null)
                } else {
                    val lang = rest.takeIf { it.isNotEmpty() }
                    // A fence inside a list item is indented with it: that
                    // much comes off each line of the code.
                    val indent = raw.length - raw.trimStart().length
                    // Only a bare fence at least as long closes it: a
                    // ```python inside a ````-block is part of the code.
                    fun closes(l: String) = l.trim().let { t -> t.length >= ticks && t.all { it == '`' } }
                    val code = ArrayList<String>()
                    i++
                    while (i < lines.size && !closes(lines[i])) {
                        val l = lines[i++]
                        code += if (l.take(indent).isBlank()) l.drop(indent) else l
                    }
                    out += Block.Code(code.joinToString("\n").trimEnd(), lang)
                }
            }
            line.isEmpty() -> flush()
            HEADING.matches(line) -> {
                flush()
                val m = HEADING.find(line)!!
                out += Block.Heading(m.groupValues[1].length, m.groupValues[2])
            }
            RULE.matches(line) -> { flush(); out += Block.Rule }
            line.startsWith(">") -> {
                flush()
                val quote = ArrayList<String>()
                while (i < lines.size && lines[i].trim().startsWith(">")) quote += lines[i++].trim().removePrefix(">").trim()
                out += Block.Quote(quote.joinToString("\n"))
                continue
            }
            line.startsWith("|") || headsPipelessTable(lines, i) -> {
                flush()
                // With a leading pipe every row has one; without, a row is any
                // line with a column break in it, up to the first that has none.
                val leading = line.startsWith("|")
                val rows = ArrayList<List<String>>()
                var header = false
                while (i < lines.size) {
                    val r = lines[i].trim()
                    if (leading && !r.startsWith("|")) break
                    val cells = tableCells(r)
                    if (!leading && rows.isNotEmpty() && cells.size < 2) break
                    when {
                        // `|-|-|` separates only right under the header; further
                        // down, a row of single dashes is somebody's empty cells.
                        rows.size == 1 && isSeparatorRow(cells, minDashes = 1) -> header = true
                        isSeparatorRow(cells, minDashes = 2) -> {}
                        else -> rows += cells
                    }
                    i++
                }
                if (rows.isNotEmpty()) out += Block.Table(rows, header)
                continue
            }
            ITEM.matches(raw) -> {
                flush()
                val items = ArrayList<Block.Item>()
                val indents = ArrayList<Int>()
                while (i < lines.size) {
                    val m = ITEM.find(lines[i])
                    if (m != null) {
                        val marker = m.groupValues[2].let { if (it in setOf("-", "*", "+", "•")) "•" else it }
                        items += Block.Item(marker, 0, m.groupValues[3])
                        indents += indentWidth(m.groupValues[1])
                        i++
                    } else if (lines[i].trim().startsWith("```")) {
                        // A code block under an item: the fence is its own
                        // block (as a "continuation line" it showed as text,
                        // and YAML's `- name:` lines inside became bullets).
                        break
                    } else if (lines[i].isNotBlank() && lines[i].first().isWhitespace() && items.isNotEmpty()) {
                        // A wrapped item: its continuation line.
                        val last = items.removeAt(items.size - 1)
                        items += last.copy(text = last.text + "\n" + lines[i++].trim())
                    } else break
                }
                val depths = listDepths(indents)
                out += Block.ListBlock(items.mapIndexed { n, item -> item.copy(depth = depths[n]) })
                continue
            }
            else -> para += line
        }
        i++
    }
    flush()
    return out
}

/** How far a list marker is indented, in columns: a tab runs to the next multiple of four. */
internal fun indentWidth(prefix: String): Int {
    var w = 0
    for (c in prefix) w = if (c == '\t') (w / 4 + 1) * 4 else w + 1
    return w
}

/**
 * Each list item's nesting depth, from how far its marker is indented: how
 * many shallower indents are open above it. Not indent / 2 — lists indented
 * by four spaces came out two levels deep per level, and by tabs (one
 * character each) flat.
 */
internal fun listDepths(indents: List<Int>): List<Int> {
    val open = ArrayList<Int>()
    return indents.map { w ->
        while (open.isNotEmpty() && open.last() > w) open.removeAt(open.size - 1)
        if (open.isEmpty() || open.last() < w) open += w
        open.size - 1
    }
}

/**
 * A table row's cells. A pipe splits columns unless it is escaped (`\|`, which
 * becomes a plain pipe) or inside a `code span`: `split('|')` cut those in two
 * and every later cell of the row moved one column over.
 */
internal fun tableCells(row: String): List<String> {
    val s = row.trim()
    val code = codeSpanEnds(s)
    val cells = ArrayList<String>()
    val cell = StringBuilder()
    var i = if (s.startsWith("|")) 1 else 0
    var closed = true
    while (i < s.length) {
        val c = s[i]
        closed = false
        when {
            c == '\\' && i + 1 < s.length -> {
                // Other escapes are the cell's own (inline) business.
                if (s[i + 1] == '|') cell.append('|') else cell.append(c).append(s[i + 1])
                i += 2
            }
            c == '`' && code[i] > 0 -> {
                cell.append(s.substring(i, code[i]).replace("\\|", "|"))
                i = code[i]
            }
            c == '|' -> {
                cells += cell.toString().trim()
                cell.clear()
                closed = true
                i++
            }
            else -> { cell.append(c); i++ }
        }
    }
    // The row's last pipe closes its last cell; without one, what's left is a cell.
    if (!closed) cells += cell.toString().trim()
    return cells
}

/** `---`, `:--`, `:-:` in every cell: the line under a table's header. */
private fun isSeparatorRow(cells: List<String>, minDashes: Int): Boolean =
    cells.isNotEmpty() && cells.all { c ->
        val dashes = c.removePrefix(":").removeSuffix(":")
        dashes.length >= minDashes && dashes.all { it == '-' }
    }

/**
 * A table written without leading pipes (`Name | Value` over `--- | ---`):
 * recognised by its header row and a separator row of as many columns.
 */
private fun headsPipelessTable(lines: List<String>, i: Int): Boolean {
    val next = lines.getOrNull(i + 1)?.trim() ?: return false
    if ('|' !in lines[i] || '|' !in next || '-' !in next) return false
    val separator = tableCells(next)
    return separator.size >= 2 && isSeparatorRow(separator, minDashes = 1) && tableCells(lines[i]).size == separator.size
}

/**
 * A table as aligned columns of monospace text. Cells are inline Markdown:
 * links stay tappable, and the columns are measured by what the reader sees,
 * not by the markup.
 */
internal fun tableText(table: Block.Table, colors: InlineColors): AnnotatedString {
    val rows = table.rows.map { r -> r.map { inline(it, colors, grid = true) } }
    val cols = rows.maxOfOrNull { it.size } ?: 0
    val widths = IntArray(cols) { c -> rows.maxOf { it.getOrNull(c)?.length ?: 0 } }
    return buildAnnotatedString {
        rows.forEachIndexed { n, row ->
            if (n > 0) append('\n')
            val last = row.indexOfLast { it.isNotEmpty() }
            val header = table.header && n == 0
            if (header) pushStyle(SpanStyle(fontWeight = FontWeight.SemiBold))
            for (c in 0..last) {
                if (c > 0) append("  ")
                append(row[c])
                if (c < last) repeat(widths[c] - row[c].length) { append(' ') }
            }
            if (header) pop()
        }
    }
}

internal data class InlineColors(val code: Color, val link: TextLinkStyles)

/**
 * Inline Markdown as styled text: code, bold, italics (`*x*` and `_x_`),
 * strike-through, links and bare URLs, nested the way they are written
 * (`**Fixed `foo()`**`, `**[PR 12](https://…)**`), with backslash escapes.
 * [grid]: for a table's monospace columns — code keeps the text's size.
 */
internal fun inline(text: String, colors: InlineColors, grid: Boolean = false): AnnotatedString = buildAnnotatedString {
    text.lines().forEachIndexed { i, line ->
        if (i > 0) append('\n')
        // Most lines are plain words: nothing to scan for.
        if (line.none { it in INLINE_MARKS } && !line.contains("http")) append(line)
        else InlineLine(line, colors, grid).run { spans(0, line.length, 0, links = true) }
    }
}

private const val INLINE_MARKS = "\\`*_~[<"

/** What a backslash makes literal: Markdown's own punctuation (`C:\Users` keeps its backslash). */
private const val ESCAPABLE = "\\`*_{}[]()<>#+-.!|~"

/** Emphasis inside emphasis inside a link…: past this depth it is shown as written. */
private const val MAX_INLINE_DEPTH = 6

/** `*`, `_`, `~` × one, two or three of them. */
private const val DELIMITER_KINDS = 9

/** Characters a bare URL can't hold. */
private fun endsUrl(c: Char) = c.isWhitespace() || c == '<' || c == '>' || c == '[' || c == ']'

/**
 * Where each code span ends: for a backtick run that opens one, the index
 * just past its closing run (a run of the same length); 0 everywhere else.
 * Code spans bind first — nothing inside one is markup.
 */
private fun codeSpanEnds(s: String): IntArray {
    val ends = IntArray(s.length)
    if ('`' !in s) return ends
    // Run lengths with no closing run further on: looked for once, not once
    // per opener (a long line of stray backticks would be quadratic).
    val unclosed = HashSet<Int>()
    var i = 0
    while (i < s.length) {
        val c = s[i]
        if (c == '\\' && i + 1 < s.length) { i += 2; continue }
        if (c != '`') { i++; continue }
        var j = i
        while (j < s.length && s[j] == '`') j++
        val len = j - i
        var close = -1
        if (len !in unclosed) {
            var k = j
            while (k < s.length && close < 0) {
                if (s[k] != '`') { k++; continue }
                var e = k
                while (e < s.length && s[e] == '`') e++
                if (e - k == len) close = k
                k = e
            }
            if (close < 0) unclosed += len
        }
        if (close >= 0) {
            ends[i] = close + len
            i = close + len
        } else i = j
    }
    return ends
}

/**
 * One line being turned into spans. Everything that would otherwise be looked
 * for again from every position — where a bracket closes, where the next
 * space is, whether a `*` has a partner at all — is worked out once per line,
 * or remembered when it fails: agents paste minified output, and the regexes
 * this replaces were quadratic in the line (measured: 4 s for 20 KB of `[`,
 * so over a minute for 100 KB, on the main thread).
 */
private class InlineLine(private val s: String, private val colors: InlineColors, private val grid: Boolean) {
    private val code = codeSpanEnds(s)

    /** For `[` and `(`: where its partner is, or -1. */
    private val partner = IntArray(s.length) { -1 }

    /** The next index at or after each one holding something a URL can't (the line's length when none). */
    private val urlStop = IntArray(s.length + 1)

    /** The next whitespace at or after each index (the line's length when none). */
    private val space = IntArray(s.length + 1)

    /** Closer searches that found nothing, per depth and delimiter: from where, and up to where. */
    private val failedFrom = Array(MAX_INLINE_DEPTH + 1) { IntArray(DELIMITER_KINDS) { Int.MAX_VALUE } }
    private val failedEnd = Array(MAX_INLINE_DEPTH + 1) { IntArray(DELIMITER_KINDS) }

    init {
        val brackets = ArrayList<Int>()
        val parens = ArrayList<Int>()
        var i = 0
        while (i < s.length) {
            val c = s[i]
            when {
                code[i] > 0 -> { i = code[i]; continue }
                c == '\\' && i + 1 < s.length -> { i += 2; continue }
                c == '[' -> brackets += i
                c == '(' -> parens += i
                c == ']' && brackets.isNotEmpty() -> partner[brackets.removeAt(brackets.size - 1)] = i
                c == ')' && parens.isNotEmpty() -> partner[parens.removeAt(parens.size - 1)] = i
            }
            i++
        }
        urlStop[s.length] = s.length
        space[s.length] = s.length
        for (k in s.length - 1 downTo 0) {
            urlStop[k] = if (endsUrl(s[k])) k else urlStop[k + 1]
            space[k] = if (s[k].isWhitespace()) k else space[k + 1]
        }
    }

    private fun codeStyle() =
        // Inline code: monospace, a little smaller, in its own colour. Not a background:
        // Compose paints span backgrounds the full line height, which read as blocks.
        if (grid) SpanStyle(fontFamily = FontFamily.Monospace, color = colors.code)
        else SpanStyle(fontFamily = FontFamily.Monospace, fontSize = 0.88.em, color = colors.code)

    /** Append `s[start, end)` with its markup turned into spans. [links]: off inside a link's own label. */
    fun AnnotatedString.Builder.spans(start: Int, end: Int, depth: Int, links: Boolean) {
        if (depth > MAX_INLINE_DEPTH) {
            append(s, start, end)
            return
        }
        var from = start
        var i = start
        while (i < end) {
            val c = s[i]
            // Where the span found at `i` ends, when one is: what lies before it is plain text.
            var next = -1
            when {
                c == '\\' -> if (i + 1 < end && s[i + 1] in ESCAPABLE) {
                    append(s, from, i)
                    append(s[i + 1])
                    next = i + 2
                }
                c == '`' -> {
                    var j = i
                    while (j < end && s[j] == '`') j++
                    val close = code[i]
                    if (close in 1..end) {
                        append(s, from, i)
                        val ticks = j - i
                        var a = j
                        var b = close - ticks
                        // "`` `x` ``": one space of padding each side isn't part of the code.
                        if (b - a > 2 && s[a] == ' ' && s[b - 1] == ' ') { a++; b-- }
                        withStyle(codeStyle()) { append(s, a, b) }
                        next = close
                    } else {
                        // Stray backticks: as written.
                        i = j
                        continue
                    }
                }
                c == '[' && links -> {
                    val label = partner[i]
                    val open = label + 1
                    val close = if (label > i + 1 && open < end && s[open] == '(') partner[open] else -1
                    val url = if (close in open until end) destination(open + 1, close) else null
                    if (url != null) {
                        append(s, from, i)
                        val labelFrom = i + 1
                        withLink(LinkAnnotation.Url(url, colors.link)) { spans(labelFrom, label, depth + 1, links = false) }
                        next = close + 1
                    }
                }
                (c == 'h' || c == '<') && links -> {
                    // <https://…> is the link without its angle brackets.
                    val at = if (c == '<') i + 1 else i
                    val stop = if (at < end) urlEnd(at, end) else -1
                    val bracketed = c == '<' && stop in 0 until end && s[stop] == '>'
                    if (stop > 0 && (c == 'h' || bracketed)) {
                        append(s, from, i)
                        val url = s.substring(at, stop)
                        withLink(LinkAnnotation.Url(url, colors.link)) { append(url) }
                        next = if (bracketed) stop + 1 else stop
                    }
                }
                c == '*' || c == '_' || c == '~' -> {
                    var j = i
                    while (j < end && s[j] == c) j++
                    val run = j - i
                    // ~~strike~~ only (a lone ~ is "about", or a home directory); up to ***both***.
                    val want = if (c == '~') (if (run == 2) 2 else 0) else if (run <= 3) run else 0
                    val close = if (want > 0 && opens(c, want, i, j, end)) closer(c, want, j, end, depth) else -1
                    if (close >= 0) {
                        append(s, from, i)
                        withStyle(emphasis(c, want)) { spans(j, close, depth + 1, links) }
                        next = close + want
                    } else if (run != 3 || c == '~') {
                        i = j
                        continue
                    }
                    // else "***bold** then italic*": the first star stands alone, the other two get their turn.
                }
            }
            if (next >= 0) {
                i = next
                from = next
            } else i++
        }
        append(s, from, end)
    }

    private fun emphasis(c: Char, count: Int) = when {
        c == '~' -> SpanStyle(textDecoration = TextDecoration.LineThrough)
        count == 1 -> SpanStyle(fontStyle = FontStyle.Italic)
        count == 2 -> SpanStyle(fontWeight = FontWeight.SemiBold)
        else -> SpanStyle(fontWeight = FontWeight.SemiBold, fontStyle = FontStyle.Italic)
    }

    /** `_`, and a single `*`, keep to word boundaries: `snake_case_names` and `2*3*4` are not emphasis. */
    private fun wordBound(c: Char, count: Int) = c == '_' || (c == '*' && count == 1)

    /**
     * Whether the run `s[i, j)` can open emphasis: something follows it
     * directly ("2 * 3" is arithmetic), and it doesn't start inside a word
     * where that matters.
     */
    private fun opens(c: Char, want: Int, i: Int, j: Int, end: Int): Boolean {
        if (j >= end || s[j].isWhitespace()) return false
        return !(wordBound(c, want) && i > 0 && s[i - 1].isLetterOrDigit())
    }

    /**
     * Where the closing delimiter starts, or -1. A run of exactly [want]
     * closes; failing that, the tail of a longer one ("*a **b***"). It closes
     * on a non-space, like it opens — "*.kt and *.kts" is two globs, not a
     * phrase in italics with its stars eaten.
     */
    private fun closer(c: Char, want: Int, from: Int, end: Int, depth: Int): Int {
        val kind = (if (c == '*') 0 else if (c == '_') 1 else 2) * 3 + (want - 1)
        // Nothing was found from further left up to at least as far right: nothing here either.
        for (d in 0..depth) if (from >= failedFrom[d][kind] && end <= failedEnd[d][kind]) return -1
        val bound = wordBound(c, want)
        var longer = -1
        var k = from
        while (k < end) {
            val ch = s[k]
            when {
                ch == '\\' && k + 1 < end && s[k + 1] in ESCAPABLE -> k += 2
                ch == '`' && code[k] in 1..end -> k = code[k]
                ch == c -> {
                    var e = k
                    while (e < end && s[e] == c) e++
                    val closes = e - k >= want && k > from && !s[k - 1].isWhitespace() &&
                        !(bound && e < s.length && s[e].isLetterOrDigit())
                    if (closes && e - k == want) return k
                    if (closes && longer < 0) longer = e - want
                    k = e
                }
                else -> k++
            }
        }
        if (longer < 0) {
            failedFrom[depth][kind] = from
            failedEnd[depth][kind] = end
        }
        return longer
    }

    /**
     * A link's destination, between its parentheses `s[a, b)`: an http(s) URL,
     * which a "title" may follow. Null when it isn't one — the text then
     * shows as written.
     */
    private fun destination(a: Int, b: Int): String? {
        var from = a
        while (from < b && s[from] == ' ') from++
        if (!s.startsWith("http://", from, ignoreCase = true) && !s.startsWith("https://", from, ignoreCase = true)) return null
        val to = minOf(space[from], b)
        if (to < b) {
            // Only a quoted title may follow the URL.
            var t = to
            while (t < b && s[t].isWhitespace()) t++
            var last = b - 1
            while (last > t && s[last].isWhitespace()) last--
            val quoted = t < last && (s[t] == '"' || s[t] == '\'') && s[last] == s[t]
            if (t < b && !quoted) return null
        }
        return s.substring(from, to)
    }

    /**
     * Where the bare URL starting at [at] ends, or -1 when there isn't one.
     * Balanced parentheses belong to it (…/wiki/Foo_(bar)); the sentence's own
     * closing one, and the full stop after it, do not.
     */
    private fun urlEnd(at: Int, end: Int): Int {
        val scheme = when {
            s.startsWith("https://", at) -> 8
            s.startsWith("http://", at) -> 7
            else -> return -1
        }
        var j = at + scheme
        while (j < end) {
            val c = s[j]
            if (c == ')' || endsUrl(c)) break
            if (c == '(') {
                // Its partner, with nothing a URL can't hold in between — or the URL ends here.
                val close = partner[j]
                if (close < 0 || close >= end || urlStop[j] < close) break
                j = close + 1
            } else j++
        }
        while (j > at + scheme && s[j - 1] in ".,;:!?'\"") j--
        return if (j > at + scheme) j else -1
    }
}
