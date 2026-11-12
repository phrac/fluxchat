package org.fluxchat.companion

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import androidx.biometric.BiometricPrompt
import org.fluxchat.companion.AppLock
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35]) // the newer sandbox image breaks on the build host's JDK
class AppLockTest {
    private val context: Context by lazy { ApplicationProvider.getApplicationContext() }

    @Before
    fun reset() {
        context.getSharedPreferences("applock", Context.MODE_PRIVATE).edit().clear().commit()
        AppLock.enabled.value = false
        AppLock.locked.value = false
        AppLock.autoPrompt.value = false
    }

    @Test fun `off by default and backgrounding does not lock`() {
        AppLock.init(context)
        assertFalse(AppLock.enabled.value)
        assertFalse(AppLock.locked.value)
        AppLock.onBackground()
        assertFalse(AppLock.locked.value)
    }

    @Test fun `enabling does not lock the app you are standing in`() {
        AppLock.init(context)
        AppLock.setEnabled(context, true)
        assertTrue(AppLock.enabled.value)
        assertFalse(AppLock.locked.value)
    }

    @Test fun `backgrounding locks once enabled`() {
        AppLock.init(context)
        AppLock.setEnabled(context, true)
        AppLock.onBackground()
        assertTrue(AppLock.locked.value)
    }

    @Test fun `a cold start with the lock saved comes up locked`() {
        AppLock.setEnabled(context, true)
        AppLock.onBackground()
        AppLock.init(context) // process restart
        assertTrue(AppLock.locked.value)
    }

    @Test fun `a detour we start (camera, picker) does not lock`() {
        AppLock.init(context)
        AppLock.setEnabled(context, true)
        AppLock.hop()
        AppLock.onBackground() // off to the camera app
        assertFalse(AppLock.locked.value)
        AppLock.onForeground() // back with the photo
        assertFalse(AppLock.locked.value)
    }

    @Test fun `the detour pass is one-shot, the next backgrounding locks`() {
        AppLock.init(context)
        AppLock.setEnabled(context, true)
        AppLock.hop()
        AppLock.onBackground()
        AppLock.onForeground()
        AppLock.onBackground() // home button after coming back
        assertTrue(AppLock.locked.value)
    }

    @Test fun `disabling unlocks and stops background re-locking`() {
        AppLock.setEnabled(context, true)
        AppLock.onBackground()
        assertTrue(AppLock.locked.value)
        AppLock.setEnabled(context, false)
        assertFalse(AppLock.locked.value)
        AppLock.onBackground()
        assertFalse(AppLock.locked.value)
    }

    @Test fun `arriving at a locked app owes a prompt, an unlocked one does not`() {
        AppLock.init(context)
        AppLock.onForeground()
        assertFalse(AppLock.autoPrompt.value)
        AppLock.setEnabled(context, true)
        AppLock.onBackground()
        assertTrue(AppLock.autoPrompt.value)
    }

    @Test fun `a prompt you dismissed is not raised again until you come back`() {
        AppLock.setEnabled(context, true)
        AppLock.onBackground()
        AppLock.promptFailed(BiometricPrompt.ERROR_USER_CANCELED)
        assertFalse(AppLock.autoPrompt.value)
        assertTrue(AppLock.locked.value)
        // Left with the lock screen up, came back: it asks by itself again.
        AppLock.onBackground()
        AppLock.onForeground()
        assertTrue(AppLock.autoPrompt.value)
    }

    @Test fun `a prompt the system cancelled is owed again, a few times`() {
        AppLock.setEnabled(context, true)
        AppLock.onBackground()
        repeat(AppLock.MAX_AUTO_RETRIES) {
            AppLock.autoPrompt.value = false // the lock screen raised it
            AppLock.promptFailed(BiometricPrompt.ERROR_CANCELED)
            assertTrue(AppLock.autoPrompt.value)
            assertEquals(AppLock.RETRY_DELAY_MS, AppLock.promptDelayMs())
        }
        AppLock.promptFailed(BiometricPrompt.ERROR_CANCELED)
        assertFalse(AppLock.autoPrompt.value)
        // A new arrival starts over, and its first prompt doesn't wait.
        AppLock.onForeground()
        assertTrue(AppLock.autoPrompt.value)
        assertEquals(0L, AppLock.promptDelayMs())
    }

    @Test fun `turning the lock off owes nothing`() {
        AppLock.setEnabled(context, true)
        AppLock.onBackground()
        AppLock.setEnabled(context, false)
        assertFalse(AppLock.autoPrompt.value)
    }
}
