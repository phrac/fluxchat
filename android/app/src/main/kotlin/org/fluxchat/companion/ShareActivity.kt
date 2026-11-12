package org.fluxchat.companion

import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.fluxchat.companion.ui.FluxTheme
import org.fluxchat.companion.ui.ShareCopy
import org.fluxchat.companion.ui.ShareInbox
import org.fluxchat.companion.ui.ShareProgress

/**
 * Takes a share from another app: copies what was shared (its read grant is
 * this activity's), then opens the main activity on "Send to…". A separate
 * activity because sharing into the main one while its task exists only
 * brings that task to the front, without the share.
 *
 * It is transparent, and while a large file copies it used to show nothing
 * at all: a frozen-looking phone. Now a card with the progress and Cancel
 * comes up when the copy takes more than a moment. A share that can't be
 * read says so here, over the app it came from, instead of bringing
 * fluxchat to the front with nothing to send.
 */
class ShareActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val share = intent
        val progress = MutableStateFlow<ShareCopy?>(null)
        val copying = lifecycleScope.launch {
            withContext(Dispatchers.IO) { ShareInbox.sweep(applicationContext) }
            val shared = ShareInbox.fromIntent(applicationContext, share) { progress.value = it }
            when {
                shared == null -> {
                    Diag.note("share: nothing in it to send (${share.action}, ${share.type})")
                    say("Nothing in that share fluxchat can send")
                }
                // Only a file, and it couldn't be read: there is nothing to pick a project for.
                shared.text == null && shared.attachment == null -> say(shared.problem ?: "The shared file couldn't be read.")
                else -> {
                    ShareInbox.offer(shared)
                    startActivity(
                        Intent(this@ShareActivity, MainActivity::class.java)
                            .putExtra(MainActivity.EXTRA_SHARE, true)
                            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP),
                    )
                }
            }
            finish()
        }
        setContent {
            FluxTheme {
                ShareProgress(progress) {
                    // Stops the copy and removes what was copied so far.
                    copying.cancel()
                    finish()
                }
            }
        }
    }

    private fun say(text: String) = Toast.makeText(applicationContext, text, Toast.LENGTH_LONG).show()
}
