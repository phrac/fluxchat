package org.fluxchat.companion.matrix

import android.content.Context

/** What is known about this phone and the recovery key. */
data class RecoveryStatus(
    /** The last check that could tell found past conversations locked: this phone isn't verified and the account has a recovery key. */
    val needed: Boolean = false,
    /** You said "Not now" on this phone. */
    val skipped: Boolean = false,
) {
    /** Worth offering unasked (the Projects card, the prompt at start): needed, and not turned down. */
    val offer: Boolean get() = needed && !skipped
}

/**
 * When the recovery-key screen shows. The check behind it is network-bound
 * (the SDK's end-to-end set-up has to finish first), and it used to run
 * *before* the app opened: on a bad connection a perfectly good saved session
 * sat on the splash screen, with every push and notification answer queued
 * behind it. So a saved session opens at once and the answer arrives later —
 * and that late answer must not pull the app out from under you: switching to
 * the prompt replaces the whole screen (the open session, a half-typed
 * request). The rules:
 *
 *  - A new sign-in waits for the answer (briefly): nothing is on screen yet,
 *    and the prompt is the natural next step.
 *  - A saved session opens on what the last check found ([atRestore]): the
 *    prompt straight away if it was needed and never answered, the app
 *    otherwise. No network either way.
 *  - An answer that arrives while the app is open ([onAnswer]) never puts the
 *    prompt up. It is remembered, the Projects screen offers it, and the next
 *    start opens on it. It can only take the prompt *away* (this phone was
 *    verified from another device in the meantime).
 *  - "Not now" is remembered per account until sign-out; Settings is the way
 *    back ([Matrix.openRecovery]).
 */
object RecoveryPrompt {
    /**
     * Whether the recovery key is wanted; null when that can't be told yet.
     * [recoverable]: the account has a key to ask for (null: not known —
     * offline at start, the SDK hasn't heard from the server). Reading
     * "not known" as "no" would dismiss a prompt that is still due and
     * forget that it was.
     */
    fun needs(verified: Boolean, recoverable: Boolean?): Boolean? = if (verified) false else recoverable

    /** A saved session's first screen, from what the last check found. */
    fun atRestore(status: RecoveryStatus): Auth = if (status.offer) Auth.NeedsRecovery else Auth.Ready

    /** Where sign-in stands once the check has answered, given what is [shown] now. */
    fun onAnswer(shown: Auth, needs: Boolean, skipped: Boolean): Auth = when {
        // Nothing to unlock (any more): a prompt still up has no job left.
        !needs -> if (shown == Auth.NeedsRecovery) Auth.Ready else shown
        skipped -> shown
        // A sign-in in progress: nothing is on screen to lose.
        shown == Auth.Starting || shown == Auth.SignedOut -> Auth.NeedsRecovery
        // In use (Ready), or already asking: left alone.
        else -> shown
    }
}

/** The recovery answers that outlive the process, per account; gone at sign-out. */
class RecoveryPrefs(context: Context) {
    private val prefs = context.getSharedPreferences("recovery", Context.MODE_PRIVATE)

    fun status(user: String) = RecoveryStatus(prefs.getBoolean("needed:$user", false), prefs.getBoolean("skipped:$user", false))

    fun setNeeded(user: String, needed: Boolean) = prefs.edit().putBoolean("needed:$user", needed).apply()

    fun setSkipped(user: String, skipped: Boolean) = prefs.edit().putBoolean("skipped:$user", skipped).apply()

    /** A new sign-in is a new device: whatever an earlier one answered doesn't apply. */
    fun forget(user: String) = prefs.edit().remove("needed:$user").remove("skipped:$user").apply()

    fun clear() = prefs.edit().clear().apply()
}
