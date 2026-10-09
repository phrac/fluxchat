package org.fluxchat.companion

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.fluxchat.companion.ui.ShareInbox

/**
 * Takes a share from another app: copies what was shared (its read grant is
 * this activity's), then opens the main activity on "Send to…". A separate
 * activity because sharing into the main one while its task exists only
 * brings that task to the front, without the share.
 */
class ShareActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val share = intent
        ShareInbox.pending.value = null
        lifecycleScope.launch {
            ShareInbox.pending.value = withContext(Dispatchers.IO) { ShareInbox.fromIntent(applicationContext, share) }
            startActivity(
                Intent(this@ShareActivity, MainActivity::class.java)
                    .putExtra(MainActivity.EXTRA_SHARE, true)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP),
            )
            finish()
        }
    }
}
