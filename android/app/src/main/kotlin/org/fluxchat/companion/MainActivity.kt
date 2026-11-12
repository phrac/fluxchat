package org.fluxchat.companion

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.view.WindowManager
import android.widget.Toast
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import org.fluxchat.companion.matrix.Auth
import org.fluxchat.companion.matrix.Matrix
import org.fluxchat.companion.ui.AppRoot
import org.fluxchat.companion.ui.FluxTheme
import org.fluxchat.companion.ui.Screen
import org.fluxchat.companion.ui.ShareInbox

/**
 * BiometricPrompt needs a FragmentActivity (it hosts its credential
 * fragment); ComponentActivity isn't one.
 */
class MainActivity : FragmentActivity() {
    /** A screen a notification asked for. */
    private val opened = MutableStateFlow<Screen?>(null)

    private val askNotifications = registerForActivityResult(ActivityResultContracts.RequestPermission()) {}

    override fun onCreate(savedInstanceState: Bundle?) {
        // The splash stays until we know whether there's a session.
        installSplashScreen().setKeepOnScreenCondition { Matrix.auth.value == Auth.Starting }
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        if (savedInstanceState == null) opened.value = screenOf(intent)
        if (Build.VERSION.SDK_INT >= 33 &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            askNotifications.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
        // The screenshot/recents guard follows the setting live.
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.STARTED) {
                AppLock.enabled.collect { applySecure() }
            }
        }
        setContent {
            FluxTheme {
                AppRoot(opened)
            }
        }
    }

    override fun onResume() {
        super.onResume()
        applySecure()
    }

    // On screen or not, for the notifications: none for the session in front
    // of you, and opening one takes its notifications down.
    override fun onStart() {
        super.onStart()
        org.fluxchat.companion.push.Attention.visible.value = true
    }

    override fun onStop() {
        org.fluxchat.companion.push.Attention.visible.value = false
        super.onStop()
    }

    /** With the lock on, content stays out of screenshots and recents. */
    private fun applySecure() {
        if (AppLock.enabled.value) {
            window.setFlags(
                WindowManager.LayoutParams.FLAG_SECURE,
                WindowManager.LayoutParams.FLAG_SECURE,
            )
        } else {
            window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        opened.value = screenOf(intent)
    }

    private fun screenOf(intent: Intent?): Screen? {
        // Reopened from the recents list, Android hands back the intent the
        // task was first started with: a notification tapped days ago would
        // open its session again instead of where you left off.
        if (intent == null || intent.flags and Intent.FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY != 0) return null
        // ShareActivity has put what was shared in ShareInbox. Taken once: the
        // intent stays the task's, and a recreated app would open the picker again.
        if (intent.getBooleanExtra(EXTRA_SHARE, false)) {
            intent.removeExtra(EXTRA_SHARE)
            if (ShareInbox.pending.value != null) return Screen.Share
            // ShareActivity only sends us here with something in the inbox;
            // gone since (it was cancelled on the way), say so rather than
            // just coming to the front.
            Diag.note("share: nothing waiting when the app opened for it")
            Toast.makeText(this, "That share is gone. Share it again.", Toast.LENGTH_LONG).show()
            return null
        }
        val room = intent.getStringExtra(EXTRA_ROOM) ?: return null
        val thread = intent.getStringExtra(EXTRA_THREAD)
        return if (thread != null) Screen.Thread(room, thread) else Screen.Room(room)
    }

    companion object {
        const val EXTRA_ROOM = "room"
        const val EXTRA_THREAD = "thread"
        const val EXTRA_SHARE = "share"
    }
}
