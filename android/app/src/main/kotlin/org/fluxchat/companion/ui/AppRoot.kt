package org.fluxchat.companion.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.runtime.toMutableStateList
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.unit.dp
import androidx.fragment.app.FragmentActivity
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import org.fluxchat.companion.AppLock
import org.fluxchat.companion.matrix.Auth
import org.fluxchat.companion.matrix.Matrix

/** The app's screens, as a back stack. */
sealed interface Screen {
    data object Rooms : Screen
    /** A project; [event] opens the timeline scrolled to that message. */
    data class Room(val roomId: String, val event: String? = null) : Screen
    data class Thread(val roomId: String, val root: String, val event: String? = null) : Screen
    /** A project's (or session's) media gallery. */
    data class Gallery(val roomId: String, val root: String?) : Screen
    data object Agents : Screen
    /** Run history across rooms, with spend per agent. */
    data object History : Screen
    /** Find a session by what it was about, across every project. */
    data object Search : Screen
    data object Settings : Screen
    data object Diagnostics : Screen
    /** Pick a room for something shared from another app. */
    data object Share : Screen
}

@Composable
fun AppRoot(opened: MutableStateFlow<Screen?>) {
    val auth by Matrix.auth.collectAsState()
    Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.surface) {
        AnimatedContent(auth, transitionSpec = { fadeIn(tween(250)) togetherWith fadeOut(tween(200)) }, label = "auth") { a ->
            when (a) {
                Auth.Starting -> Box(Modifier.fillMaxSize())
                Auth.SignedOut -> LoginScreen()
                Auth.NeedsRecovery -> RecoveryScreen()
                Auth.RestoreFailed -> RestoreFailedScreen()
                Auth.Ready -> {
                    val locked by AppLock.locked.collectAsState()
                    // The lock COVERS the app, never replaces it: the nav
                    // stack and half-typed composers must survive, and a
                    // camera/picker hop re-arms the lock on the way back.
                    Box(Modifier.fillMaxSize()) {
                        Navigator(opened)
                        if (locked) LockScreen()
                    }
                }
            }
        }
    }
}

/**
 * The biometric gate. The system prompt goes up by itself on every arrival
 * ([AppLock.autoPrompt]) — but only once this window has focus: before that
 * the app is still behind the keyguard or sliding in from the launcher, and
 * the system cancels a prompt raised then. If the prompt is dismissed, this
 * screen is the retry.
 */
@Composable
private fun LockScreen() {
    val activity = LocalContext.current as? FragmentActivity
    val owed by AppLock.autoPrompt.collectAsState()
    val focused = LocalWindowInfo.current.isWindowFocused
    Column(
        Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).clickable(enabled = true, onClick = {}),
        Arrangement.Center,
        Alignment.CenterHorizontally,
    ) {
        Icon(
            Icons.Outlined.Lock,
            null,
            modifier = Modifier.size(56.dp),
            tint = MaterialTheme.colorScheme.primary,
        )
        Spacer(Modifier.size(16.dp))
        Text("fluxchat is locked", style = MaterialTheme.typography.titleMedium)
        Spacer(Modifier.size(16.dp))
        Button(onClick = { activity?.let { AppLock.unlock(it) } }) { Text("Unlock") }
    }
    // Back must not pop the screens hidden underneath: it leaves the app.
    BackHandler { activity?.moveTaskToBack(true) }
    LaunchedEffect(owed, focused) {
        if (owed && focused) {
            delay(AppLock.promptDelayMs())
            activity?.let { AppLock.unlock(it) }
        }
    }
}

/** A saved session that wouldn't restore (transient, usually): retry in place. */
@Composable
private fun RestoreFailedScreen() {
    val error by Matrix.error.collectAsState()
    Column(Modifier.fillMaxSize(), Arrangement.Center, Alignment.CenterHorizontally) {
        Text(
            error ?: "Couldn't reopen your sessions.",
            style = MaterialTheme.typography.bodyLarge,
            modifier = Modifier.padding(horizontal = 32.dp),
        )
        Spacer(Modifier.size(16.dp))
        Button(onClick = { Matrix.launch { Matrix.ensureStarted() } }) { Text("Try again") }
        // The way out when it never will (the phone's keystore lost the key):
        // signing in keeps the old store aside until the new sign-in works.
        TextButton(onClick = { Matrix.signInAgain() }) { Text("Sign in again") }
    }
}

/** A screen as text, so the back stack survives the activity being recreated. */
internal fun encodeScreen(s: Screen): String? = when (s) {
    Screen.Rooms -> "projects"
    is Screen.Room -> listOf("project", s.roomId, s.event.orEmpty()).joinToString(SCREEN_SEP)
    is Screen.Thread -> listOf("thread", s.roomId, s.root, s.event.orEmpty()).joinToString(SCREEN_SEP)
    is Screen.Gallery -> listOf("gallery", s.roomId, s.root.orEmpty()).joinToString(SCREEN_SEP)
    Screen.Agents -> "agents"
    Screen.History -> "history"
    Screen.Search -> "search"
    Screen.Settings -> "settings"
    Screen.Diagnostics -> "diagnostics"
    // What was shared lives in memory only: nothing to come back to.
    Screen.Share -> null
}

internal fun decodeScreen(text: String): Screen? {
    val p = text.split(SCREEN_SEP)
    fun at(i: Int) = p.getOrNull(i)?.takeIf { it.isNotEmpty() }
    return when (p[0]) {
        "projects" -> Screen.Rooms
        "project" -> at(1)?.let { Screen.Room(it, at(2)) }
        "thread" -> at(1)?.let { room -> at(2)?.let { Screen.Thread(room, it, at(3)) } }
        "gallery" -> at(1)?.let { Screen.Gallery(it, at(2)) }
        "agents" -> Screen.Agents
        "history" -> Screen.History
        "search" -> Screen.Search
        "settings" -> Screen.Settings
        "diagnostics" -> Screen.Diagnostics
        else -> null
    }
}

/** Not in a room or event ID. */
private const val SCREEN_SEP = "\n"

private val StackSaver = listSaver<SnapshotStateList<Screen>, String>(
    save = { stack -> stack.mapNotNull(::encodeScreen) },
    restore = { saved ->
        val screens = saved.mapNotNull(::decodeScreen)
        (if (screens.firstOrNull() == Screen.Rooms) screens else listOf(Screen.Rooms) + screens).toMutableStateList()
    },
)

@Composable
private fun Navigator(opened: MutableStateFlow<Screen?>) {
    // Saved: the stack was a plain `remember`, so turning the phone (or a
    // dark-mode switch, a font-size change, unfolding it) in a session
    // dropped you back on Projects.
    val stack = rememberSaveable(saver = StackSaver) { mutableStateListOf<Screen>(Screen.Rooms) }
    // +1 forward, -1 back: which way screens slide.
    val direction = remember { mutableIntStateOf(1) }
    val holder = rememberSaveableStateHolder()
    // A screen that has left the stack takes its saved state with it. Kept,
    // the next opening of the same session came up at the old scroll
    // position — somewhere in its history instead of at the new answer.
    val forget: (Screen) -> Unit = { s -> if (s !in stack) holder.removeState(s.toString()) }
    val open by opened.collectAsState()
    LaunchedEffect(open) {
        open?.let { s ->
            val gone = stack.filter { it != Screen.Rooms }
            stack.removeAll { it != Screen.Rooms }
            gone.forEach(forget)
            if (s is Screen.Thread) stack.add(Screen.Room(s.roomId))
            direction.intValue = 1
            stack.add(s)
            opened.value = null
        }
    }
    val go: (Screen) -> Unit = { direction.intValue = 1; stack.add(it) }
    val back: () -> Unit = {
        if (stack.size > 1) {
            direction.intValue = -1
            forget(stack.removeAt(stack.size - 1))
        }
    }
    // Replace the top screen (one session for another, from the sessions list).
    // Onto the screen right below it, that is just going back: swapping left
    // the same project twice in the stack, and Back then changed nothing.
    val swap: (Screen) -> Unit = {
        if (stack.size > 1 && stack[stack.size - 2] == it) back()
        else { direction.intValue = 1; stack[stack.size - 1] = it }
    }
    BackHandler(enabled = stack.size > 1) { back() }
    AnimatedContent(
        targetState = stack.last(),
        transitionSpec = {
            val d = direction.intValue
            (slideInHorizontally(tween(300)) { w -> d * w / 4 } + fadeIn(tween(300))) togetherWith
                (slideOutHorizontally(tween(300)) { w -> -d * w / 4 } + fadeOut(tween(200)))
        },
        label = "screens",
    ) { s ->
        // Screens keep their scroll position etc. when you come back to them.
        holder.SaveableStateProvider(s.toString()) { Screen(s, go, back, swap) }
    }
}

@Composable
private fun Screen(s: Screen, go: (Screen) -> Unit, back: () -> Unit, swap: (Screen) -> Unit) {
        when (s) {
            Screen.Rooms -> RoomsScreen(go)
            is Screen.Room -> ChatScreen(s.roomId, null, go, back, swap, jumpTo = s.event)
            is Screen.Thread -> ChatScreen(s.roomId, s.root, go, back, swap, jumpTo = s.event)
            is Screen.Gallery -> GalleryScreen(s.roomId, s.root, go, back)
            Screen.Agents -> AgentsScreen(back)
            Screen.History -> HistoryScreen(go, back)
            Screen.Search -> SearchScreen(go, back, swap)
            Screen.Settings -> SettingsScreen(go, back)
            Screen.Diagnostics -> DiagnosticsScreen(back)
            Screen.Share -> ShareScreen(back, swap)
        }
}
