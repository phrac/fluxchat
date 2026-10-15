package org.fluxchat.companion

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.view.WindowManager
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
        // ShareActivity has put what was shared in ShareInbox. Taken once: the
        // intent stays the task's, and a recreated app would open the picker again.
        if (intent?.getBooleanExtra(EXTRA_SHARE, false) == true) {
            intent.removeExtra(EXTRA_SHARE)
            return if (ShareInbox.pending.value != null) Screen.Share else null
        }
        val room = intent?.getStringExtra(EXTRA_ROOM) ?: return null
        val thread = intent.getStringExtra(EXTRA_THREAD)
        return if (thread != null) Screen.Thread(room, thread) else Screen.Room(room)
    }

    companion object {
        const val EXTRA_ROOM = "room"
        const val EXTRA_THREAD = "thread"
        const val EXTRA_SHARE = "share"
    }
}
