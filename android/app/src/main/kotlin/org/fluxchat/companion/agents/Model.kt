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
) {
    val isLocal: Boolean get() = id.startsWith("~")
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
