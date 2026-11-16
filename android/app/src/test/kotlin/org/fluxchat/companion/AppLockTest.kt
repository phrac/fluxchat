package org.fluxchat.companion

import android.app.KeyguardManager
import android.content.ActivityNotFoundException
import android.content.Context
import androidx.activity.result.ActivityResultRegistry
import androidx.activity.result.contract.ActivityResultContract
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.app.ActivityOptionsCompat
import androidx.fragment.app.FragmentActivity
import androidx.test.core.app.ApplicationProvider
import androidx.biometric.BiometricPrompt
import org.fluxchat.companion.AppLock
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
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
        AppLock.hopOver()
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

    @Test fun `a picker drawn over the app does not excuse the next leaving`() {
        // The photo picker is a sheet over the app: paused, never stopped, so
        // nothing ever "came back" to spend the pass. Home some time later
        // was then taken for the detour, and the app stayed open behind it.
        AppLock.init(context)
        AppLock.setEnabled(context, true)
        AppLock.hop()
        AppLock.hopOver() // the main activity is in front again, picture picked
        AppLock.onBackground()
        assertTrue(AppLock.locked.value)
    }

    @Test fun `a launch that opened nothing gives its pass back`() {
        AppLock.init(context)
        AppLock.setEnabled(context, true)
        val noCameraApp = object : ActivityResultRegistry() {
            override fun <I, O> onLaunch(requestCode: Int, contract: ActivityResultContract<I, O>, input: I, options: ActivityOptionsCompat?) {
                throw ActivityNotFoundException("no camera app")
            }
        }
        val launcher = noCameraApp.register("shot", ActivityResultContracts.GetContent()) {}
        // The caller still hears of it (it says "No camera app on this phone").
        assertThrows(ActivityNotFoundException::class.java) { launcher.launchHopping("image/*") }
        AppLock.onBackground() // Home: not a detour of ours
        assertTrue(AppLock.locked.value)
    }

    @Test fun `a detour come back from in time does not ask`() {
        AppLock.init(context)
        AppLock.setEnabled(context, true)
        AppLock.hop(now = 1_000)
        AppLock.onBackground(now = 2_000)
        AppLock.onForeground(now = 2_000 + AppLock.HOP_PASS_MS / 2)
        assertFalse(AppLock.locked.value)
        assertFalse(AppLock.autoPrompt.value)
    }

    @Test fun `a detour never come back from locks on the way back`() {
        // Home out of the camera instead of taking the photo, the phone put
        // down: whoever opens the app an hour later is asked.
        AppLock.init(context)
        AppLock.setEnabled(context, true)
        AppLock.hop(now = 1_000)
        AppLock.onBackground(now = 2_000)
        assertFalse(AppLock.locked.value)
        AppLock.onForeground(now = 1_000 + AppLock.HOP_PASS_MS + 1)
        assertTrue(AppLock.locked.value)
        assertTrue(AppLock.autoPrompt.value)
    }

    @Test fun `a pass nobody used runs out`() {
        // A detour started somewhere that neither left nor reported back:
        // its pass is no good for a leaving long after.
        AppLock.init(context)
        AppLock.setEnabled(context, true)
        AppLock.hop(now = 1_000)
        AppLock.onBackground(now = 1_000 + AppLock.HOP_PASS_MS + 1)
        assertTrue(AppLock.locked.value)
    }

    @Test fun `an overdue detour changes nothing with the lock off`() {
        AppLock.init(context)
        AppLock.hop(now = 1_000)
        AppLock.onBackground(now = 2_000)
        AppLock.onForeground(now = 1_000 + AppLock.HOP_PASS_MS + 1)
        assertFalse(AppLock.locked.value)
    }

    @Test fun `with the phone's screen lock gone the app opens instead of locking you out`() {
        // Lock on, then the phone's PIN removed in the system's settings: the
        // prompt has nothing to ask with and failed every time, with the
        // switch that turns the lock off behind it.
        val keyguard = shadowOf(context.getSystemService(KeyguardManager::class.java))
        keyguard.setIsDeviceSecure(true)
        assertTrue(AppLock.canAsk(context))
        keyguard.setIsDeviceSecure(false)
        assertFalse(AppLock.canAsk(context))

        AppLock.setEnabled(context, true)
        AppLock.onBackground()
        assertTrue(AppLock.locked.value)
        val activity = Robolectric.buildActivity(FragmentActivity::class.java).setup().get()
        var opened: Boolean? = null
        AppLock.unlock(activity) { opened = it }
        assertEquals(true, opened)
        assertFalse(AppLock.locked.value)
        assertFalse(AppLock.autoPrompt.value)
        // The setting is yours: it locks again once the phone has a screen lock.
        assertTrue(AppLock.enabled.value)
    }
}
