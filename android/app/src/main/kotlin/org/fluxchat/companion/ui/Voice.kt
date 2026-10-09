package org.fluxchat.companion.ui

import android.content.ActivityNotFoundException
import android.content.Intent
import android.speech.RecognizerIntent
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext

/** Android's speech input for a prompt: it listens (no mic permission of ours), we get the words. */
private fun speechIntent(prompt: String) = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH)
    .putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
    .putExtra(RecognizerIntent.EXTRA_PROMPT, prompt)

/**
 * Dictation: call the returned function to listen; [onText] gets what was
 * said. Null when the phone has no speech input (no button then).
 */
@Composable
fun rememberDictation(prompt: String = "Ask your agent", onText: (String) -> Unit): (() -> Unit)? {
    val context = LocalContext.current
    val available = remember { context.packageManager.resolveActivity(speechIntent(prompt), 0) != null }
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.StartActivityForResult()) { r ->
        r.data?.getStringArrayListExtra(RecognizerIntent.EXTRA_RESULTS)?.firstOrNull()?.takeIf { it.isNotBlank() }?.let(onText)
    }
    if (!available) return null
    return {
        try {
            launcher.launch(speechIntent(prompt))
        } catch (_: ActivityNotFoundException) {
            Toast.makeText(context, "No speech input on this phone", Toast.LENGTH_SHORT).show()
        }
    }
}

/** Dictated words added to what's typed: a space between, or the words alone. */
fun withDictation(text: String, said: String): String =
    if (text.isBlank()) said.replaceFirstChar { it.uppercase() } else text.trimEnd() + " " + said
