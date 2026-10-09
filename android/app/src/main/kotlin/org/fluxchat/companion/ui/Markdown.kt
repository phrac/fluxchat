package org.fluxchat.companion.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** A message body: Markdown as agents write it (fenced code, inline styles, links, headings). */
@Composable
fun MarkdownText(
    body: String,
    color: Color = MaterialTheme.colorScheme.onSurface,
    maxLines: Int = Int.MAX_VALUE,
    style: androidx.compose.ui.text.TextStyle = MaterialTheme.typography.bodyMedium,
) {
    val blocks = remember(body) { splitBlocks(body) }
    val codeBg = MaterialTheme.colorScheme.surfaceContainerHighest
    val link = MaterialTheme.colorScheme.primary
    val colors = InlineColors(codeBg, TextLinkStyles(SpanStyle(color = link, textDecoration = TextDecoration.Underline)))
    SelectionContainer {
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            var linesLeft = maxLines
            for (b in blocks) {
                if (linesLeft <= 0) break
                when (b) {
                    is Block.Code -> {
                        Text(
                            b.text,
                            fontFamily = FontFamily.Monospace,
                            fontSize = 12.sp,
                            color = MaterialTheme.colorScheme.onSurface,
                            softWrap = false,
                            maxLines = linesLeft,
                            modifier = Modifier.fillMaxWidth()
                                .background(codeBg, RoundedCornerShape(10.dp))
                                .horizontalScroll(rememberScrollState())
                                .padding(10.dp),
                        )
                        linesLeft -= b.text.lines().size
                    }
                    is Block.Prose -> {
                        val text = remember(b.text, colors) { inline(b.text, colors) }
                        Text(text, color = color, style = style, maxLines = linesLeft)
                        linesLeft -= b.text.lines().size
                    }
                }
            }
        }
    }
}

private sealed interface Block {
    data class Prose(val text: String) : Block
    data class Code(val text: String) : Block
}

private fun splitBlocks(body: String): List<Block> {
    val out = ArrayList<Block>()
    val prose = StringBuilder()
    val code = StringBuilder()
    var inCode = false
    for (line in body.lines()) {
        if (line.trimStart().startsWith("```")) {
            if (inCode) {
                out += Block.Code(code.toString().trimEnd('\n'))
                code.clear()
            } else if (prose.isNotBlank()) {
                out += Block.Prose(prose.toString().trim('\n'))
                prose.clear()
            } else {
                prose.clear()
            }
            inCode = !inCode
            continue
        }
        (if (inCode) code else prose).append(line).append('\n')
    }
    if (inCode && code.isNotEmpty()) out += Block.Code(code.toString().trimEnd('\n'))
    if (prose.isNotBlank()) out += Block.Prose(prose.toString().trim('\n'))
    return out
}

private val LINK = Regex("""\[([^\]]+)]\((https?://[^)\s]+)\)""")
private val URL = Regex("""https?://[^\s<>()\[\]]+[^\s<>()\[\].,;:!?'"]""")
private val INLINE = Regex("""`([^`]+)`|\*\*([^*]+)\*\*|__([^_]+)__|(?<![\w*])\*([^*\s][^*]*)\*(?!\w)|~~([^~]+)~~""")

private data class InlineColors(val codeBg: Color, val link: TextLinkStyles)

private fun inline(text: String, colors: InlineColors): AnnotatedString = buildAnnotatedString {
    text.lines().forEachIndexed { i, rawLine ->
        if (i > 0) append('\n')
        var line = rawLine
        val heading = Regex("^(#{1,6})\\s+").find(line)
        if (heading != null) {
            line = line.substring(heading.range.last + 1)
            withStyle(SpanStyle(fontWeight = FontWeight.Bold, fontSize = (17 - heading.groupValues[1].length).sp)) { spans(line, colors) }
        } else {
            if (line.trimStart().startsWith("- ") || line.trimStart().startsWith("* ")) {
                val indent = line.length - line.trimStart().length
                line = " ".repeat(indent) + "• " + line.trimStart().substring(2)
            }
            spans(line, colors)
        }
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
                    g[1].isNotEmpty() -> withStyle(SpanStyle(fontFamily = FontFamily.Monospace, background = colors.codeBg)) { append(g[1]) }
                    g[2].isNotEmpty() -> withStyle(SpanStyle(fontWeight = FontWeight.Bold)) { append(g[2]) }
                    g[3].isNotEmpty() -> withStyle(SpanStyle(fontWeight = FontWeight.Bold)) { append(g[3]) }
                    g[4].isNotEmpty() -> withStyle(SpanStyle(fontStyle = FontStyle.Italic)) { append(g[4]) }
                    g[5].isNotEmpty() -> withStyle(SpanStyle(textDecoration = TextDecoration.LineThrough)) { append(g[5]) }
                }
            }
        }
        pos = next.range.last + 1
    }
    if (pos < line.length) append(line.substring(pos))
}
