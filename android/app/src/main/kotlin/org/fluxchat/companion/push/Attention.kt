package org.fluxchat.companion.push

import android.content.Context
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.launchIn
import kotlinx.coroutines.flow.onEach
import org.fluxchat.companion.AppLock
import org.fluxchat.companion.matrix.Matrix
import org.fluxchat.companion.ui.Screen

/**
 * What you are looking at right now: a notification about the session on
 * screen is noise, and one about a session you then open has done its job.
 */
object Attention {
    /** The screen on top of the main activity's stack (the navigator sets it); null when there is none. */
    val top = MutableStateFlow<Screen?>(null)

    /**
     * Whether the main activity is on screen. Not `App.foreground`: the quick
     * prompt and the share sheet are activities of ours too, up over some
     * other app while the session underneath is nowhere to be seen.
     */
    val visible = MutableStateFlow(false)

    /** Report the screen on top (one call in the navigator). */
    @Composable
    fun Track(screen: Screen) {
        DisposableEffect(screen) {
            top.value = screen
            onDispose { if (top.value == screen) top.value = null }
        }
    }

    /**
     * Whether [top] shows the message [eventId] (in [threadRoot]'s session, or
     * the project itself). A session's screen shows everything in it. A
     * project's screen shows its own messages and each session's latest
     * answer — enough for news, not for a prompt: an approval asked inside a
     * session can't be answered from the project, so it still [needsAnswer].
     */
    fun covers(top: Screen?, roomId: String, threadRoot: String?, eventId: String?, needsAnswer: Boolean): Boolean = when (top) {
        is Screen.Thread -> top.roomId == roomId && (top.root == threadRoot || top.root == eventId)
        is Screen.Room -> top.roomId == roomId && (threadRoot == null || !needsAnswer)
        else -> false
    }

    /** [covers], while the app is actually in front of you and unlocked. */
    fun watching(
        top: Screen?, visible: Boolean, locked: Boolean,
        roomId: String, threadRoot: String?, eventId: String?, needsAnswer: Boolean,
    ): Boolean = visible && !locked && covers(top, roomId, threadRoot, eventId, needsAnswer)

    fun watching(roomId: String, threadRoot: String?, eventId: String?, needsAnswer: Boolean): Boolean =
        watching(top.value, visible.value, AppLock.locked.value, roomId, threadRoot, eventId, needsAnswer)

    /** A session or project coming into view takes its notifications down. */
    fun start(context: Context) {
        val app = context.applicationContext
        combine(top, visible, AppLock.locked) { screen, shown, locked -> screen.takeIf { shown && !locked } }
            .distinctUntilChanged()
            .onEach { screen -> if (screen != null) Notifier.cancelSeen(app, screen) }
            .launchIn(Matrix.scope)
    }
}
