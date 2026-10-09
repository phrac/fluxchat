package org.fluxchat.companion

import android.app.Application
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import org.fluxchat.companion.matrix.Matrix

class App : Application() {
    override fun onCreate() {
        super.onCreate()
        Matrix.init(this)
        Notifications.createChannels(this)
        Matrix.launch { Matrix.ensureStarted() }
        org.fluxchat.companion.push.PromptWatcher.start(this)
        ProcessLifecycleOwner.get().lifecycle.addObserver(object : DefaultLifecycleObserver {
            override fun onStart(owner: LifecycleOwner) {
                foreground = true
                Matrix.resumed()
            }

            override fun onStop(owner: LifecycleOwner) {
                foreground = false
            }
        })
    }

    companion object {
        @Volatile var foreground = false
            private set
    }
}
