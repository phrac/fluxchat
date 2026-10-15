package org.fluxchat.companion

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.fluxchat.companion.AppLock
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

    @Test fun `disabling unlocks and stops background re-locking`() {
        AppLock.setEnabled(context, true)
        AppLock.onBackground()
        assertTrue(AppLock.locked.value)
        AppLock.setEnabled(context, false)
        assertFalse(AppLock.locked.value)
        AppLock.onBackground()
        assertFalse(AppLock.locked.value)
    }
}
