package org.fluxchat.companion

import android.app.KeyguardManager
import android.content.Context
import android.os.SystemClock
import androidx.activity.result.ActivityResultLauncher
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

    /**
     * Set while WE start a trip out (camera, photo/file picker). Coming back
     * from our own detour shouldn't make you prove who you are again — you
     * never handed the phone to anyone. One-shot: cleared on return, so any
     * other backgrounding (home, app switch, screen off) still locks.
     */
    @Volatile private var hopping = false

    /** When the detour began ([SystemClock.elapsedRealtime]): its pass runs out [HOP_PASS_MS] later. */
    @Volatile private var hoppedAt = 0L

    /**
     * How long a detour's pass is good for, from the tap that started it. A
     * pass with no end let anyone in: Home out of the camera instead of
     * taking the photo, and the app stayed unlocked behind it for as long as
     * it took someone to open it again. Past this, coming back asks like any
     * other return — the screen you left, attachment included, is still
     * there behind the lock.
     */
    const val HOP_PASS_MS = 5 * 60_000L

    private fun onPass(now: Long) = hopping && now - hoppedAt <= HOP_PASS_MS

    /**
     * Whether the lock screen should raise the system prompt by itself, the
     * moment its window can show one. Armed on every arrival at a locked app
     * (cold start, coming back from the background), spent when the prompt
     * goes up. The lock screen used to ask once, when it was first drawn —
     * which is while the app is still on its way to the front (behind the
     * keyguard, or mid-transition from the launcher). The system cancels a
     * prompt raised then, so you landed on "fluxchat is locked" and had to tap
     * Unlock to get the prompt you came for.
     */
    val autoPrompt = MutableStateFlow(false)

    /** System-cancelled prompts re-raised since the last arrival. */
    private var autoRetries = 0

    /** How often a prompt the *system* cancelled is raised again unasked. */
    const val MAX_AUTO_RETRIES = 2

    /** A cancelled prompt is retried after the transition that cancelled it. */
    const val RETRY_DELAY_MS = 400L

    /** How long the lock screen waits before raising the prompt it owes. */
    fun promptDelayMs(): Long = if (autoRetries > 0) RETRY_DELAY_MS else 0

    private fun arm() {
        autoRetries = 0
        autoPrompt.value = locked.value
    }

    /** Mark the upcoming backgrounding as our own picker/camera detour. */
    fun hop(now: Long = SystemClock.elapsedRealtime()) {
        hopping = true
        hoppedAt = now
    }

    /**
     * The detour is over and the app never left the foreground for it: the
     * photo picker is a sheet drawn over the app (paused, never stopped), and
     * a camera that isn't installed opens nothing at all. Neither is ever
     * "come back from", so the pass stayed in hand — and the next real
     * leaving (Home, another app) was waved through as "our own detour",
     * with the app unlocked behind it. Called when the main activity is in
     * front again, and when a launch fails.
     */
    fun hopOver() {
        hopping = false
    }

    /**
     * The app came to the front: the detour pass is spent, and a lock screen
     * that's waiting (even one you dismissed the prompt on before leaving)
     * asks again by itself. A detour its pass ran out on is a return like
     * any other: it locks.
     */
    fun onForeground(now: Long = SystemClock.elapsedRealtime()) {
        if (hopping && enabled.value && !onPass(now)) locked.value = true
        hopping = false
        arm()
    }

    fun init(context: Context) {
        val on = prefs(context).getBoolean(KEY_ENABLED, false)
        enabled.value = on
        // A cold start with the lock on is a lock: nothing shows until unlock.
        locked.value = on
        arm()
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
        if (!on) {
            locked.value = false
            autoPrompt.value = false
        }
    }

    /** The last activity stopped: content must not come back without an unlock. */
    fun onBackground(now: Long = SystemClock.elapsedRealtime()) {
        if (enabled.value && !onPass(now)) {
            locked.value = true
            arm()
        }
    }

    /**
     * A prompt ended without unlocking. One the system took down
     * (ERROR_CANCELED: the app wasn't in front yet, the screen went off) is
     * owed again; one *you* dismissed is not — that would be a prompt you
     * can't get rid of. The lock screen's button is the retry then.
     */
    internal fun promptFailed(errorCode: Int) {
        if (errorCode == BiometricPrompt.ERROR_CANCELED && locked.value && autoRetries < MAX_AUTO_RETRIES) {
            autoRetries++
            autoPrompt.value = true
        } else {
            autoPrompt.value = false
        }
    }

    private fun unlocked() {
        locked.value = false
        autoPrompt.value = false
        autoRetries = 0
    }

    /** Unlock with a fingerprint/face/credential prompt. */
    fun unlock(activity: FragmentActivity, onDone: (Boolean) -> Unit = {}) {
        if (!enabled.value) {
            unlocked()
            onDone(true)
            return
        }
        // The phone's own screen lock was removed since the lock was turned
        // on: the prompt has nothing left to ask with and fails every time,
        // and the switch that turns the lock off is behind it — the app could
        // only be opened again by clearing its data, session and keys with
        // it. Removing a screen lock takes the old one, so it was you.
        if (!canAsk(activity)) {
            Diag.note("app lock: this phone has no screen lock any more, opening without asking")
            unlocked()
            onDone(true)
            return
        }
        // BiometricPrompt drops a request made while the activity's state is
        // saved (stopped) without telling anyone: stay armed for the return.
        if (activity.supportFragmentManager.isStateSaved) {
            onDone(false)
            return
        }
        autoPrompt.value = false
        BiometricPrompt(
            activity,
            ContextCompat.getMainExecutor(activity),
            object : BiometricPrompt.AuthenticationCallback() {
                override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                    unlocked()
                    onDone(true)
                }

                // Canceled / no hardware: stay locked; the screen offers a retry.
                override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                    // A prompt you (or the system) took down is ordinary; one
                    // that can't be shown at all is why the app won't open.
                    if (errorCode !in DISMISSALS) Diag.error("app lock: the unlock prompt failed ($errorCode: $errString)")
                    promptFailed(errorCode)
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

    /** Whether the phone still has a screen lock to ask for (a fingerprint or a face needs one too). */
    internal fun canAsk(context: Context): Boolean =
        context.getSystemService(KeyguardManager::class.java)?.isDeviceSecure != false

    private val DISMISSALS = setOf(
        BiometricPrompt.ERROR_CANCELED,
        BiometricPrompt.ERROR_USER_CANCELED,
        BiometricPrompt.ERROR_NEGATIVE_BUTTON,
    )

    private fun prefs(context: Context) =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
}

/** `launch()` that tells the lock this detour is ours (camera, pickers). */
fun <I> ActivityResultLauncher<I>.launchHopping(input: I) {
    AppLock.hop()
    try {
        launch(input)
    } catch (e: Exception) {
        // Nothing opened (no camera app on the phone): no detour to excuse.
        AppLock.hopOver()
        throw e
    }
}
