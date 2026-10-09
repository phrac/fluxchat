package org.fluxchat.companion.quick

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import org.fluxchat.companion.MainActivity
import org.fluxchat.companion.ui.FluxTheme
import org.fluxchat.companion.ui.QuickPromptSheet

/** The quick prompt sheet, over whatever you were doing (its own task, so the app doesn't come up). */
class QuickPromptActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val room = intent.getStringExtra(Quick.EXTRA_ROOM)
        val voice = intent.getBooleanExtra(Quick.EXTRA_VOICE, false)
        setContent {
            FluxTheme {
                QuickPromptSheet(
                    roomArg = room,
                    voice = voice,
                    onOpen = { id ->
                        startActivity(
                            Intent(this, MainActivity::class.java)
                                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
                                .putExtra(MainActivity.EXTRA_ROOM, id),
                        )
                        finish()
                    },
                    onDone = { finish() },
                )
            }
        }
    }
}
