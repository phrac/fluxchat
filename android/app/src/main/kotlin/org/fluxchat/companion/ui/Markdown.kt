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
                    is Block.Code -> CodeBlock(b.text, b.lang, maxLines = linesLeft)
                    is Block.Table -> CodeBlock(b.text, null, maxLines = linesLeft, copy = false)
                }
                linesLeft -= b.lines
            }
        }
    }
}

/** Fenced code: its language and a copy button over a scrollable monospace body. */
@Composable
private fun CodeBlock(code: String, lang: String?, maxLines: Int, copy: Boolean = true) {
    val scheme = MaterialTheme.colorScheme
    val clipboard = LocalClipboardManager.current
    var copied by remember { mutableStateOf(false) }
    LaunchedEffect(copied) { if (copied) { delay(1500); copied = false } }
    Surface(color = scheme.surfaceContainerHigh, shape = RoundedCornerShape(12.dp), modifier = Modifier.fillMaxWidth()) {
        Column {
            if (copy) {
                Row(Modifier.fillMaxWidth().padding(start = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(lang ?: "code", style = MaterialTheme.typography.labelSmall, color = scheme.onSurfaceVariant, modifier = Modifier.weight(1f))
                    IconButton(onClick = { clipboard.setText(AnnotatedString(code)); copied = true }, modifier = Modifier.size(36.dp)) {
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

private sealed interface Block {
    /** Lines it takes, for folding long answers. */
    val lines: Int

    data class Paragraph(val text: String) : Block { override val lines get() = text.lines().size }
    data class Heading(val level: Int, val text: String) : Block { override val lines get() = 1 }
    data class Item(val marker: String, val depth: Int, val text: String)
    data class ListBlock(val items: List<Item>) : Block { override val lines get() = items.sumOf { it.text.lines().size } }
    data class Quote(val text: String) : Block { override val lines get() = text.lines().size }
    data object Rule : Block { override val lines get() = 1 }
    data class Code(val text: String, val lang: String?) : Block { override val lines get() = text.lines().size + 1 }
    data class Table(val text: String) : Block { override val lines get() = text.lines().size }
}

private val HEADING = Regex("^(#{1,6})\\s+(.*)$")
private val ITEM = Regex("^(\\s*)([-*+•]|\\d{1,3}[.)])\\s+(.*)$")
private val RULE = Regex("^(-{3,}|\\*{3,}|_{3,})$")
private val TABLE_SEPARATOR = Regex("^\\|?\\s*:?-{2,}:?\\s*(\\|\\s*:?-{2,}:?\\s*)*\\|?$")

/** Markdown blocks, line by line (agents write simple Markdown; nothing exotic). */
private fun parseBlocks(body: String): List<Block> {
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
                val lang = line.removePrefix("```").trim().takeIf { it.isNotEmpty() }
                val code = ArrayList<String>()
                i++
                while (i < lines.size && !lines[i].trim().startsWith("```")) code += lines[i++]
                out += Block.Code(code.joinToString("\n").trimEnd(), lang)
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
            line.startsWith("|") -> {
                flush()
                val rows = ArrayList<List<String>>()
                while (i < lines.size && lines[i].trim().startsWith("|")) {
                    val r = lines[i++].trim()
                    if (!TABLE_SEPARATOR.matches(r)) rows += r.trim('|').split('|').map { it.trim().replace("**", "").replace("`", "") }
                }
                out += Block.Table(table(rows))
                continue
            }
            ITEM.matches(raw) -> {
                flush()
                val items = ArrayList<Block.Item>()
                while (i < lines.size) {
                    val m = ITEM.find(lines[i])
                    if (m != null) {
                        val marker = m.groupValues[2].let { if (it in setOf("-", "*", "+", "•")) "•" else it }
                        items += Block.Item(marker, m.groupValues[1].length / 2, m.groupValues[3])
                        i++
                    } else if (lines[i].isNotBlank() && lines[i].startsWith(" ") && items.isNotEmpty()) {
                        // A wrapped item: its continuation line.
                        val last = items.removeAt(items.size - 1)
                        items += last.copy(text = last.text + "\n" + lines[i++].trim())
                    } else break
                }
                out += Block.ListBlock(items)
                continue
            }
            else -> para += line
        }
        i++
    }
    flush()
    return out
}

/** A table's rows as aligned columns (monospace). */
private fun table(rows: List<List<String>>): String {
    val cols = rows.maxOfOrNull { it.size } ?: 0
    val widths = (0 until cols).map { c -> rows.maxOf { it.getOrNull(c)?.length ?: 0 } }
    return rows.joinToString("\n") { r -> (0 until cols).joinToString("  ") { c -> (r.getOrNull(c) ?: "").padEnd(widths[c]) }.trimEnd() }
}

private val LINK = Regex("""\[([^\]]+)]\((https?://[^)\s]+)\)""")
private val URL = Regex("""https?://[^\s<>()\[\]]+[^\s<>()\[\].,;:!?'"]""")
private val INLINE = Regex("""`([^`]+)`|\*\*([^*]+)\*\*|__([^_]+)__|(?<![\w*])\*([^*\s][^*]*)\*(?!\w)|~~([^~]+)~~""")

private data class InlineColors(val code: Color, val link: TextLinkStyles)

private fun inline(text: String, colors: InlineColors): AnnotatedString = buildAnnotatedString {
    text.lines().forEachIndexed { i, line ->
        if (i > 0) append('\n')
        spans(line, colors)
    }
}

private fun AnnotatedString.Builder.spans(line: String, colors: InlineColors) {
    var pos = 0
    while (pos < line.length) {
        val link = LINK.find(line, pos)
        val url = URL.find(line, pos)
        val style = INLINE.find(line, pos)
        val next = listOfNotNull(link, url, style).minByOrNull { it.range.first } ?: break
        append(line.substring(pos, next.range.first))
        when (next) {
            link -> withLink(LinkAnnotation.Url(next.groupValues[2], colors.link)) { append(next.groupValues[1]) }
            url -> withLink(LinkAnnotation.Url(next.value, colors.link)) { append(next.value) }
            else -> {
                val g = next.groupValues
                when {
                    // Inline code: monospace, a little smaller, in its own colour. Not a background:
                    // Compose paints span backgrounds the full line height, which read as blocks.
                    g[1].isNotEmpty() -> withStyle(SpanStyle(fontFamily = FontFamily.Monospace, fontSize = 0.88.em, color = colors.code)) {
                        append(g[1])
                    }
                    g[2].isNotEmpty() -> withStyle(SpanStyle(fontWeight = FontWeight.SemiBold)) { append(g[2]) }
                    g[3].isNotEmpty() -> withStyle(SpanStyle(fontWeight = FontWeight.SemiBold)) { append(g[3]) }
                    g[4].isNotEmpty() -> withStyle(SpanStyle(fontStyle = FontStyle.Italic)) { append(g[4]) }
                    g[5].isNotEmpty() -> withStyle(SpanStyle(textDecoration = TextDecoration.LineThrough)) { append(g[5]) }
                }
            }
        }
        pos = next.range.last + 1
    }
    if (pos < line.length) append(line.substring(pos))
}
