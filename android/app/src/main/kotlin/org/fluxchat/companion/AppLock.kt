package org.fluxchat.companion

import android.content.Context
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import kotlinx.coroutines.flow.MutableStateFlow

/**
 * Optional biometric / screen-lock gate on the app's content.
 *
 * Covers MainActivity's screens (and everything the notification deep-links
 * into): leaving the app re-locks it, and while enabled FLAG_SECURE keeps the
 * content out of screenshots and the app switcher. Quick entry points
 * (widget, Quick Settings tile, shortcuts, share) are deliberately NOT gated:
 * they only ever send text you type, never read your history.
 */
object AppLock {
    private const val PREFS = "applock"
    private const val KEY_ENABLED = "enabled"
    private val AUTHENTICATORS = BiometricManager.Authenticators.BIOMETRIC_STRONG or
        BiometricManager.Authenticators.BIOMETRIC_WEAK or
        BiometricManager.Authenticators.DEVICE_CREDENTIAL

    /** Whether the user asked for a lock. Survives restarts in prefs. */
    val enabled = MutableStateFlow(false)

    /** Whether the content currently sits behind the lock screen. */
    val locked = MutableStateFlow(false)

    fun init(context: Context) {
        val on = prefs(context).getBoolean(KEY_ENABLED, false)
        enabled.value = on
        // A cold start with the lock on is a lock: nothing shows until unlock.
        locked.value = on
    }

    fun canEnable(context: Context): Boolean =
        BiometricManager.from(context).canAuthenticate(AUTHENTICATORS) == BiometricManager.BIOMETRIC_SUCCESS

    /**
     * Turn the lock on or off. Turning it on does NOT lock the app you're
     * standing in: it takes effect the next time the app leaves the
     * foreground (and on the next cold start).
     */
    fun setEnabled(context: Context, on: Boolean) {
        prefs(context).edit().putBoolean(KEY_ENABLED, on).apply()
        enabled.value = on
        if (!on) locked.value = false
    }

    /** The last activity stopped: content must not come back without an unlock. */
    fun onBackground() {
        if (enabled.value) locked.value = true
    }

    /** Unlock with a fingerprint/face/credential prompt. */
    fun unlock(activity: FragmentActivity, onDone: (Boolean) -> Unit = {}) {
        if (!enabled.value) {
            locked.value = false
            onDone(true)
            return
        }
        BiometricPrompt(
            activity,
            ContextCompat.getMainExecutor(activity),
            object : BiometricPrompt.AuthenticationCallback() {
                override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                    locked.value = false
                    onDone(true)
                }

                // Canceled / no hardware: stay locked; the screen offers a retry.
                override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                    onDone(false)
                }
            },
        ).authenticate(
            BiometricPrompt.PromptInfo.Builder()
                .setTitle("Unlock fluxchat")
                .setSubtitle("Fingerprint, face, or screen lock")
                .setAllowedAuthenticators(AUTHENTICATORS)
                .build(),
        )
    }

    private fun prefs(context: Context) =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
}
