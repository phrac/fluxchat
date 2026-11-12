package org.fluxchat.companion.quick

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import org.fluxchat.companion.Diag
import org.fluxchat.companion.MainActivity
import org.fluxchat.companion.ui.FluxTheme
import org.fluxchat.companion.ui.QuickPromptSheet
import org.fluxchat.companion.ui.ShareInbox

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
                    onOpen = { id, draft ->
                        // What was typed goes with you: the project's message
                        // box takes it the way it takes shared text. Not while
                        // a share is waiting for its project — then it stays
                        // this sheet's draft.
                        val taken = draft.isNotBlank() && ShareInbox.handOver(id, draft.trim())
                        if (taken) Quick.saveDraft(applicationContext, id, "")
                        else if (draft.isNotBlank()) Diag.note("quick prompt: a share is waiting, the draft stays with the sheet")
                        startActivity(
                            Intent(this, MainActivity::class.java)
                                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
                                .putExtra(MainActivity.EXTRA_ROOM, id),
                        )
                        finish()
                        taken
                    },
                    onDone = { finish() },
                )
            }
        }
    }
}
