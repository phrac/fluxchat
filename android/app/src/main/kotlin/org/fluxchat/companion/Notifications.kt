package org.fluxchat.companion

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context

/** Notification channels: prompts that wait on you, and finished runs. */
object Notifications {
    const val PROMPTS = "prompts"
    const val RUNS = "runs"
    const val LIVE = "live"

    fun createChannels(context: Context) {
        val nm = context.getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(
            NotificationChannel(PROMPTS, "Agent waiting on you", NotificationManager.IMPORTANCE_HIGH).apply {
                description = "Approvals and questions from your agents"
            },
        )
        nm.createNotificationChannel(
            NotificationChannel(RUNS, "Agent messages", NotificationManager.IMPORTANCE_DEFAULT).apply {
                description = "Answers and finished runs"
            },
        )
        nm.createNotificationChannel(
            NotificationChannel(LIVE, "Agents at work", NotificationManager.IMPORTANCE_LOW).apply {
                description = "What your agents are doing right now, with Stop"
                setShowBadge(false)
            },
        )
    }
}
