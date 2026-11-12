package org.fluxchat.companion.agents

/** A run's state, as desktop fluxchat has it (agents.rs `RunState`). */
enum class RunState { Working, NeedsYou, Done, Failed;

    val live: Boolean get() = this == Working || this == NeedsYou

    val label: String
        get() = when (this) {
            Working -> "Working"
            NeedsYou -> "Needs you"
            Done -> "Done"
            Failed -> "Failed"
        }
}

data class Reaction(val emoji: String, val users: List<String>)

enum class SendState { Sent, Sending, Failed }

/** What can be done about a message of yours that hasn't gone out. */
enum class SendAction { Retry, Delete, Cancel }

/**
 * How long a message sits "sending" before Cancel is offered: every send is
 * "sending" for a moment, and a button flashing under each one would only
 * make the list jump.
 */
const val SLOW_SEND_MS = 5_000L

/**
 * What a row offers for a send in [state]: a failed one can be tried again or
 * deleted, one stuck on its way out cancelled. Only while it's still a local
 * echo ([local]) — once the server has it there is nothing left to abort.
 */
fun sendActions(state: SendState, local: Boolean, waitedMs: Long): List<SendAction> = when {
    !local -> emptyList()
    state == SendState.Failed -> listOf(SendAction.Retry, SendAction.Delete)
    state == SendState.Sending && waitedMs >= SLOW_SEND_MS -> listOf(SendAction.Cancel)
    else -> emptyList()
}

/**
 * One timeline event, as the agent logic and the UI need it (the desktop's
 * `MessageInfo`). [id] is the event ID, or `~` + transaction ID for a local
 * echo.
 */
data class Msg(
    val id: String,
    val sender: String,
    val senderName: String,
    val body: String,
    /** Milliseconds since the epoch. */
    val ts: Long,
    val isOwn: Boolean,
    val reactions: List<Reaction> = emptyList(),
    /** Root of the thread this is a reply in. */
    val threadRoot: String? = null,
    /** Replies, when this is a thread root. */
    val threadCount: Int = 0,
    val threadLatest: String? = null,
    val agent: Protocol? = null,
    val sendState: SendState = SendState.Sent,
    /** A message (not a membership / state change / unsupported event). */
    val isMessage: Boolean = true,
    val edited: Boolean = false,
    val formatted: String? = null,
    val senderAvatar: String? = null,
    /** An image: what to load (the SDK's media source JSON) and its size. */
    val media: Media? = null,
    /** A file, video or audio clip: a card to open or save. */
    val file: FileRef? = null,
    /**
     * A failed send the queue tries again by itself (the connection dropped).
     * False on a failed one: it's parked — and holds up everything sent after
     * it in the project — until it's retried or deleted.
     */
    val sendRetrying: Boolean = false,
    /** Why a send was parked, in a few words, when the SDK says. */
    val sendError: String? = null,
    /**
     * Your own reactions here (their keys) whose send failed for good: parked
     * like a message, and holding up later sends the same way, but with no
     * row of their own to retry or delete them from.
     */
    val parkedReactions: List<String> = emptyList(),
) {
    val isLocal: Boolean get() = id.startsWith("~")
    /** A send of yours that failed for good and waits for Retry or Delete. */
    val sendParked: Boolean get() = isLocal && sendState == SendState.Failed && !sendRetrying
    fun sendActions(now: Long): List<SendAction> = sendActions(sendState, isLocal, now - ts)
    val hasThread: Boolean get() = threadCount > 0 || threadLatest != null
}

data class Media(val source: String, val thumbnail: String?, val width: Int?, val height: Int?)

enum class FileKind { File, Video, Audio }

/** An attached file: the SDK's media source JSON, its name, type and size, and a video's thumbnail. */
data class FileRef(
    val source: String,
    val name: String,
    val mime: String?,
    val size: Long?,
    val kind: FileKind,
    val thumbnail: Media? = null,
    val durationMs: Long? = null,
)

/** A run: where it stands, when it was asked for and the agent's latest step. */
data class RunInfo(
    val state: RunState,
    /** The request's timestamp (ms). */
    val since: Long,
    val step: String? = null,
    val steps: Int = 0,
    val progress: Float? = null,
    val agent: String = "",
    val costUsd: Double? = null,
    val model: String? = null,
    val context: Float? = null,
    /** How the run ended, one line (protocol `result`), once it has. */
    val result: String? = null,
) {
    /** For picking between two reports on one view: the livelier. */
    fun rank(): Pair<Int, Long> = Pair(
        when (state) {
            RunState.NeedsYou -> 0
            RunState.Working -> 1
            RunState.Failed -> 2
            RunState.Done -> 3
        },
        -since,
    )
}

/** Where a run lives: a room's main timeline, or a thread in it. */
data class ViewKey(val roomId: String, val threadRoot: String? = null) {
    override fun toString() = if (threadRoot == null) roomId else "$roomId › $threadRoot"
}
