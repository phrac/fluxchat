package org.fluxchat.companion.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
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
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.fragment.app.FragmentActivity
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
                    if (locked) LockScreen() else Navigator(opened)
                }
            }
        }
    }
}

/**
 * The biometric gate. The prompt fires itself on arrival; if it's dismissed
 * or fails, this screen is the retry. Deep-linked screens wait: [opened]
 * holds the request until Navigator (unlocked) consumes it.
 */
@Composable
private fun LockScreen() {
    val activity = LocalContext.current as? FragmentActivity
    Column(Modifier.fillMaxSize(), Arrangement.Center, Alignment.CenterHorizontally) {
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
    LaunchedEffect(Unit) { activity?.let { AppLock.unlock(it) } }
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
    }
}

@Composable
private fun Navigator(opened: MutableStateFlow<Screen?>) {
    val stack = remember { mutableStateListOf<Screen>(Screen.Rooms) }
    // +1 forward, -1 back: which way screens slide.
    val direction = remember { mutableIntStateOf(1) }
    val open by opened.collectAsState()
    LaunchedEffect(open) {
        open?.let { s ->
            stack.removeAll { it != Screen.Rooms }
            if (s is Screen.Thread) stack.add(Screen.Room(s.roomId))
            direction.intValue = 1
            stack.add(s)
            opened.value = null
        }
    }
    val go: (Screen) -> Unit = { direction.intValue = 1; stack.add(it) }
    val back: () -> Unit = { if (stack.size > 1) { direction.intValue = -1; stack.removeAt(stack.size - 1) } }
    // Replace the top screen (one session for another, from the sessions list).
    val swap: (Screen) -> Unit = { direction.intValue = 1; stack[stack.size - 1] = it }
    BackHandler(enabled = stack.size > 1) { back() }
    val holder = rememberSaveableStateHolder()
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
