package org.fluxchat.companion.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.outlined.BrokenImage
import androidx.compose.material.icons.outlined.Chat
import androidx.compose.material.icons.outlined.Description
import androidx.compose.material.icons.outlined.Download
import androidx.compose.material.icons.outlined.Movie
import androidx.compose.material.icons.outlined.PlayCircle
import androidx.compose.material.icons.outlined.Share
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.async
import kotlinx.coroutines.launch
import org.fluxchat.companion.agents.FileKind
import org.fluxchat.companion.agents.GalleryTab
import org.fluxchat.companion.agents.Msg
import org.fluxchat.companion.agents.galleryMediaOf
import org.fluxchat.companion.agents.galleryMonthLabel
import org.fluxchat.companion.agents.galleryMonthSections
import org.fluxchat.companion.agents.galleryIsImage
import org.fluxchat.companion.agents.isGalleryMedia
import org.fluxchat.companion.matrix.GalleryView
import org.fluxchat.companion.matrix.Matrix

/**
 * A project's (or session's) gallery: its images and videos, and its files,
 * newest first, month by month — the phone port of the desktop's gallery
 * modal. Tap an image to browse it; a video or file opens where it was
 * posted, as on the desktop (videos play in the timeline).
 */
@Composable
@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
fun GalleryScreen(roomId: String, root: String?, go: (Screen) -> Unit, back: () -> Unit) {
    var view by remember { mutableStateOf<GalleryView?>(null) }
    var failed by remember { mutableStateOf(false) }
    var attempt by remember { mutableIntStateOf(0) }
    LaunchedEffect(roomId, root, attempt) {
        if (view == null) {
            // What this view already loaded is the seed; the server fills the rest.
            val loaded = Matrix.roomTimeline(roomId)?.messages?.value.orEmpty()
            val seed = if (root == null) loaded else loaded.filter { it.id == root || it.threadRoot == root }
            // Opened on the app's scope, not this screen's. openGallery only
            // caches a view (to be reused, and released at logout) once its
            // timeline is open; leaving the screen before that cancelled it
            // in between, with a started timeline nothing would ever close.
            // Run to the end, an opening nobody waits for lands in the cache.
            val opening = Matrix.scope.async { Matrix.openGallery(roomId, root, seed) }
            view = try {
                opening.await()
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Exception) {
                // A throw here used to take the app down (nothing above catches it).
                org.fluxchat.companion.Diag.error("Opening the gallery of ${root ?: roomId}", e)
                failed = true
                return@LaunchedEffect
            }
            if (view == null) {
                failed = true
                org.fluxchat.companion.Diag.error("Opening the gallery of ${root ?: roomId}, no timeline", null)
            }
        }
    }
    val state = view?.state?.collectAsState(null)?.value
    val tab = state?.tab ?: GalleryTab.Media

    // Opening from a session: the root's own text names it.
    val title = remember(roomId, root) {
        if (root == null) Matrix.roomName(roomId)
        else Matrix.roomTimeline(roomId)?.messages?.value?.firstOrNull { it.id == root }?.let { threadTitle(it, emptyList()) } ?: "Session"
    }

    // A thread reply found from the project opens its session first (the desktop's jump).
    val jump: (Msg) -> Unit = { m ->
        when {
            root != null -> go(Screen.Thread(roomId, root, m.id))
            m.threadRoot != null -> go(Screen.Thread(roomId, m.threadRoot, m.id))
            else -> go(Screen.Room(roomId, m.id))
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                navigationIcon = { IconButton(onClick = back) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") } },
                title = { Text("$title · Gallery", maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.titleMedium) },
            )
        },
    ) { pads ->
        Column(Modifier.padding(pads).fillMaxSize()) {
            TabRow(selectedTabIndex = if (tab == GalleryTab.Media) 0 else 1) {
                Tab(selected = tab == GalleryTab.Media, onClick = { view?.setTab(GalleryTab.Media) }, text = { Text("Media") })
                Tab(selected = tab == GalleryTab.Files, onClick = { view?.setTab(GalleryTab.Files) }, text = { Text("Files") })
            }
            val items = state?.items.orEmpty()
            val shown = remember(items, tab) { items.filter { if (tab == GalleryTab.Media) isGalleryMedia(it) else !isGalleryMedia(it) } }
            val loading = state?.loading == true
            val done = state?.done == true
            val error = state?.error
            when {
                view == null -> Column(Modifier.fillMaxSize(), Arrangement.Center, Alignment.CenterHorizontally) {
                    if (failed) {
                        Text("Couldn't open the gallery.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                        TextButton(onClick = { failed = false; attempt++ }) { Text("Try again") }
                    } else CircularProgressIndicator(Modifier.size(28.dp), strokeWidth = 2.dp)
                }
                shown.isEmpty() && error == null -> Column(Modifier.fillMaxSize(), Arrangement.Center, Alignment.CenterHorizontally) {
                    if (loading) CircularProgressIndicator(Modifier.size(28.dp), strokeWidth = 2.dp)
                    else Text("Nothing here yet.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                tab == GalleryTab.Media -> MediaTab(shown, loading, done, error, { view?.continueLoading() }, { view?.loadMore() }, jump)
                else -> FilesTab(shown, loading, done, error, { view?.continueLoading() }, { view?.loadMore() }, jump)
            }
        }
    }
}

/** A grid row: a month label or one attachment. */
private sealed interface GalleryRow {
    data class Header(val label: String, val start: Int) : GalleryRow
    data class Item(val msg: Msg) : GalleryRow
}

private fun galleryRows(items: List<Msg>): List<GalleryRow> {
    val rows = ArrayList<GalleryRow>()
    for (month in galleryMonthSections(items)) {
        rows += GalleryRow.Header(month.label, month.start)
        for (i in month.start until month.end) rows += GalleryRow.Item(items[i])
    }
    return rows
}

/** Bottom of the list reached: page more, or rest once the start is in. */
@Composable
private fun GalleryFooter(loading: Boolean, done: Boolean, error: String?, loadMore: () -> Unit) {
    when {
        // The detail went to Diag; SDK error text names its own machinery.
        error != null -> Text(
            "Couldn't load more. Tap to try again.",
            // 48 dp to tap, whatever the text's height.
            Modifier.fillMaxWidth().clickable(role = Role.Button, onClick = loadMore).heightIn(min = 48.dp).wrapContentHeight().padding(horizontal = 12.dp),
            style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.error,
        )
        loading -> Box(Modifier.fillMaxWidth().padding(16.dp), Alignment.Center) { CircularProgressIndicator(Modifier.size(24.dp), strokeWidth = 2.dp) }
        !done -> Text("Load older", Modifier.fillMaxWidth().clickable(role = Role.Button, onClick = loadMore).padding(16.dp), style = MaterialTheme.typography.labelLarge)
    }
}

@Composable
private fun MediaTab(items: List<Msg>, loading: Boolean, done: Boolean, error: String?, more: () -> Unit, loadMore: () -> Unit, jump: (Msg) -> Unit) {
    val rows = remember(items) { galleryRows(items) }
    val grid = rememberLazyGridState()
    // Approaching the end of the list pages the server (the desktop's near-end
    // rule); only the footer tap resets the burst the cap counts.
    LaunchedEffect(grid, rows.size) {
        snapshotFlow { grid.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0 }
            .collect { last -> if (rows.isNotEmpty() && last >= rows.size - 6) more() }
    }
    // Open image by event id, not index: live merges grow the list underneath.
    var openId by remember { mutableStateOf<String?>(null) }
    val images = remember(items) { items.filter { galleryIsImage(it) } }
    LazyVerticalGrid(
        GridCells.Adaptive(112.dp),
        state = grid,
        contentPadding = PaddingValues(8.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
        modifier = Modifier.fillMaxSize(),
    ) {
        // Month headings and the footer take the whole line (they were one
        // 112 dp cell each, squeezed in beside the thumbnails).
        items(rows, key = { row ->
            when (row) {
                is GalleryRow.Header -> "h:" + row.start
                is GalleryRow.Item -> row.msg.id
            }
        }, span = { row -> if (row is GalleryRow.Header) GridItemSpan(maxLineSpan) else GridItemSpan(1) }) { row ->
            when (row) {
                is GalleryRow.Header -> Text(
                    row.label, Modifier.fillMaxWidth().padding(top = 8.dp, bottom = 2.dp),
                    style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary,
                )
                is GalleryRow.Item -> GalleryTile(row.msg) {
                    if (galleryIsImage(row.msg)) openId = row.msg.id else jump(row.msg)
                }
            }
        }
        item(key = "footer", span = { GridItemSpan(maxLineSpan) }) { GalleryFooter(loading, done, error, loadMore) }
    }
    val openAt = openId?.let { id -> images.indexOfFirst { it.id == id } } ?: -1
    if (openAt >= 0) GalleryViewer(images, openAt, { i -> images.getOrNull(i)?.let { openId = it.id } }, { openId = null }, jump)
}

/** Videos and files, newest first; the card opens the file, ↩ goes to its message. */
@Composable
private fun FilesTab(items: List<Msg>, loading: Boolean, done: Boolean, error: String?, more: () -> Unit, loadMore: () -> Unit, jump: (Msg) -> Unit) {
    val list = rememberLazyListState()
    LaunchedEffect(list, items.size) {
        snapshotFlow { list.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0 }
            .collect { last -> if (items.isNotEmpty() && last >= items.size - 4) more() }
    }
    val rows = remember(items) {
        val out = ArrayList<Pair<String?, Msg>>()
        for (month in galleryMonthSections(items))
            for (i in month.start until month.end)
                out += (if (i == month.start) month.label else null) to items[i]
        out
    }
    LazyColumn(
        state = list, contentPadding = PaddingValues(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.fillMaxSize(),
    ) {
        items(rows, key = { it.second.id }) { (label, m) ->
            Column {
                label?.let {
                    Text(it, Modifier.padding(top = 6.dp, bottom = 2.dp), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
                }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    val f = m.file
                    if (f != null) {
                        FileCard(f, Modifier.weight(1f))
                        IconButton(onClick = { jump(m) }) { Icon(Icons.Outlined.Chat, "Go to message") }
                    }
                }
            }
        }
        item(key = "footer") { GalleryFooter(loading, done, error, loadMore) }
    }
}

/** One square tile: the image, or a video's thumbnail with a play mark. */
@Composable
private fun GalleryTile(m: Msg, open: () -> Unit) {
    val media = galleryMediaOf(m)
    var attempt by remember { mutableIntStateOf(0) }
    val state by produceState<Any?>(media?.let { Images.cached("thumb:" + it.source) }, m, attempt) {
        val cached = media?.let { Images.cached("thumb:" + it.source) }
        value = cached
        if (cached == null && media != null) value = Images.load(media, full = false) ?: false
    }
    val scheme = MaterialTheme.colorScheme
    val image = galleryIsImage(m)
    // An image whose thumbnail didn't load: the tap is another try (the
    // tile was dead until it had scrolled away and back).
    val retry = image && state == false
    Box(
        Modifier.fillMaxWidth().aspectRatio(1f).clip(MaterialTheme.shapes.medium)
            .background(scheme.surfaceContainerHighest)
            // Videos jump even without a thumbnail loaded, as on the desktop.
            .clickable(
                enabled = state is ImageBitmap || !image || retry,
                onClickLabel = if (retry) "try again" else if (image) "view" else "go to the message",
                onClick = { if (retry) attempt++ else open() },
            ),
        contentAlignment = Alignment.Center,
    ) {
        when (val s = state) {
            is ImageBitmap -> Image(s, galleryLabel(m), Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
            false -> Icon(Icons.Outlined.BrokenImage, "Couldn't load the image", tint = scheme.onSurfaceVariant)
            else -> if (media == null)
                Icon(Icons.Outlined.Description, galleryLabel(m), tint = scheme.onSurfaceVariant)
            else CircularProgressIndicator(Modifier.size(24.dp), strokeWidth = 2.dp)
        }
        if (m.media == null && m.file?.kind == FileKind.Video)
            Icon(Icons.Outlined.PlayCircle, null, Modifier.size(36.dp), tint = Color.White.copy(alpha = 0.85f))
    }
}

/** What a tile or the viewer shows, in words: a screen reader had nothing to say about either. */
private fun galleryLabel(m: Msg): String {
    val what = if (m.file?.kind == FileKind.Video) "Video" else "Image"
    return m.file?.name?.let { "$what $it" } ?: "$what from ${shortName(m.sender, m.senderName)}"
}

/**
 * Full-screen image browse: swipe (or ‹ ›) through the gallery, save, share,
 * ↩ goes to the message. [index] is where the open image is in [images] now;
 * [step] is told where a swipe has moved to.
 */
@Composable
private fun GalleryViewer(images: List<Msg>, index: Int, step: (Int) -> Unit, close: () -> Unit, jump: (Msg) -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val pager = rememberPagerState(initialPage = index) { images.size }
    // Zoomed in, a drag looks around the image; at rest it turns the page.
    var zoomed by remember { mutableStateOf(false) }
    val told by rememberUpdatedState(step)
    // A swipe (or ‹ ›) came to rest on another image: that is the open one
    // now, and it starts unzoomed — the page that said "zoomed" is gone.
    LaunchedEffect(pager) {
        snapshotFlow { pager.settledPage }.collect {
            zoomed = false
            told(it)
        }
    }
    // The list grew above the open image (a live merge): follow it there.
    LaunchedEffect(index) { if (index != pager.currentPage && !pager.isScrollInProgress) pager.scrollToPage(index) }
    ViewerDialog(close) {
        HorizontalPager(
            pager, Modifier.fillMaxSize(), userScrollEnabled = !zoomed,
            key = { images.getOrNull(it)?.id ?: it },
        ) { page ->
            val m = images.getOrNull(page)
            val media = m?.let { galleryMediaOf(it) }
            if (m != null && media != null) ViewerImage(media, galleryLabel(m)) { if (page == pager.currentPage) zoomed = it }
        }
        val m = images.getOrNull(pager.currentPage) ?: return@ViewerDialog
        ViewerBar(close) {
            val source = m.file?.source ?: m.media?.source
            val busy = source?.let { fileBusy(it) } ?: false
            // A file-sent image keeps its own name; a message image is named for what it is.
            val file = m.file
            ViewerButton(Icons.Outlined.Download, "Save the image", enabled = !busy) {
                if (file != null) FileJobs.save(context, file) else m.media?.let { FileJobs.saveImage(context, it) }
            }
            ViewerButton(Icons.Outlined.Share, "Share the image", enabled = !busy) {
                if (file != null) FileJobs.share(context, file) else m.media?.let { FileJobs.shareImage(context, it) }
            }
            ViewerButton(Icons.Outlined.Chat, "Go to message") { jump(m); close() }
        }
        if (pager.currentPage > 0)
            NavButton(Icons.AutoMirrored.Filled.KeyboardArrowLeft, "Previous image", Modifier.align(Alignment.CenterStart)) {
                scope.launch { pager.animateScrollToPage(pager.currentPage - 1) }
            }
        if (pager.currentPage < images.lastIndex)
            NavButton(Icons.AutoMirrored.Filled.KeyboardArrowRight, "Next image", Modifier.align(Alignment.CenterEnd)) {
                scope.launch { pager.animateScrollToPage(pager.currentPage + 1) }
            }
    }
}

@Composable
private fun NavButton(icon: ImageVector, label: String, modifier: Modifier, onClick: () -> Unit) {
    Box(
        modifier.padding(8.dp).size(56.dp).clip(MaterialTheme.shapes.extraLarge)
            .background(Color.Black.copy(alpha = 0.5f)).clickable(role = Role.Button, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) { Icon(icon, label, Modifier.size(32.dp), tint = Color.White) }
}
