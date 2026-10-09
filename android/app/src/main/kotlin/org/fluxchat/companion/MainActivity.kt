package org.fluxchat.companion

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import kotlinx.coroutines.flow.MutableStateFlow
import org.fluxchat.companion.matrix.Auth
import org.fluxchat.companion.matrix.Matrix
import org.fluxchat.companion.ui.AppRoot
import org.fluxchat.companion.ui.FluxTheme
import org.fluxchat.companion.ui.Screen

class MainActivity : ComponentActivity() {
    /** A screen a notification asked for. */
    private val opened = MutableStateFlow<Screen?>(null)

    private val askNotifications = registerForActivityResult(ActivityResultContracts.RequestPermission()) {}

    override fun onCreate(savedInstanceState: Bundle?) {
        // The splash stays until we know whether there's a session.
        installSplashScreen().setKeepOnScreenCondition { Matrix.auth.value == Auth.Starting }
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        opened.value = screenOf(intent)
        if (Build.VERSION.SDK_INT >= 33 &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            askNotifications.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
        setContent {
            FluxTheme {
                AppRoot(opened)
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        opened.value = screenOf(intent)
    }

    private fun screenOf(intent: Intent?): Screen? {
        // ShareActivity has put what was shared in ShareInbox.
        if (intent?.getBooleanExtra(EXTRA_SHARE, false) == true) return Screen.Share
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
