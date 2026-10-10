package org.fluxchat.companion

import android.app.Application
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import org.fluxchat.companion.matrix.Matrix

class App : Application() {
    override fun onCreate() {
        super.onCreate()
        // Before any client: the SDK needs the Context to verify TLS certificates.
        runCatching { org.matrix.rustsdk.Android.init(this) }
            .onFailure { Diag.error("Initializing the SDK for Android (TLS will fail)", it) }
        Matrix.init(this)
        org.fluxchat.companion.Update.init(this)
        Notifications.createChannels(this)
        Matrix.launch { Matrix.ensureStarted() }
        Matrix.launch { org.fluxchat.companion.Update.check() }
        org.fluxchat.companion.push.PromptWatcher.start(this)
        org.fluxchat.companion.push.LiveRuns.start(this)
        org.fluxchat.companion.quick.Quick.start(this)
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
