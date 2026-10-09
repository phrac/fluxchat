package org.fluxchat.companion.agents

import android.content.Context
import org.tomlj.Toml
import org.tomlj.TomlArray
import org.tomlj.TomlTable

data class Choice(val emoji: String, val label: String)
data class MenuItem(val label: String, val command: String)
data class CommandSpec(val name: String, val description: String)

/**
 * What an agent harness does on Matrix (the desktop's harness.rs; the
 * profiles are the same TOML files as `assets/agents`, copied in at build).
 */
data class Harness(
    val id: String = "generic",
    val name: String = "Generic agent",
    val commandPrefix: String = "/",
    val mentionCommands: Boolean = false,
    val stop: String = "",
    val steer: String = "",
    val queue: String = "",
    val workingReaction: String = "",
    val doneReaction: String = "",
    val failedReaction: String = "",
    val typingMeansWorking: Boolean = true,
    val replyInThread: Boolean = false,
    val progressSteps: Boolean = true,
    val runtimeFooter: Boolean = false,
    val questionPrefix: String = "",
    val promptOverMarkers: List<String> = emptyList(),
    val approvals: List<Choice> = emptyList(),
    val menu: List<MenuItem> = emptyList(),
    val commands: List<CommandSpec> = emptyList(),
) {
    /** A command as it goes out: `!stop`. */
    fun command(name: String) = commandPrefix + name

    fun hasCommand(name: String) = commands.any { it.name == name }

    companion object {
        const val DEFAULT_ID = "hermes"

        fun parse(text: String): Harness {
            val t = Toml.parse(text)
            val d = Harness()
            fun s(k: String, def: String) = t.getString(k) ?: def
            fun b(k: String, def: Boolean) = t.getBoolean(k) ?: def
            fun tables(k: String): List<TomlTable> {
                val arr: TomlArray = t.getArray(k) ?: return emptyList()
                return (0 until arr.size()).mapNotNull { i -> val tt: TomlTable? = arr.getTable(i); tt }
            }
            fun strings(k: String): List<String> {
                val arr: TomlArray = t.getArray(k) ?: return emptyList()
                return (0 until arr.size()).mapNotNull { i -> val v: String? = arr.getString(i); v }
            }
            return Harness(
                id = s("id", d.id),
                name = s("name", d.name),
                commandPrefix = s("command_prefix", d.commandPrefix),
                mentionCommands = b("mention_commands", d.mentionCommands),
                stop = s("stop", d.stop),
                steer = s("steer", d.steer),
                queue = s("queue", d.queue),
                workingReaction = s("working_reaction", d.workingReaction),
                doneReaction = s("done_reaction", d.doneReaction),
                failedReaction = s("failed_reaction", d.failedReaction),
                typingMeansWorking = b("typing_means_working", d.typingMeansWorking),
                replyInThread = b("reply_in_thread", d.replyInThread),
                progressSteps = b("progress_steps", d.progressSteps),
                runtimeFooter = b("runtime_footer", d.runtimeFooter),
                questionPrefix = s("question_prefix", d.questionPrefix),
                promptOverMarkers = strings("prompt_over_markers"),
                approvals = tables("approvals").map { Choice(it.getString("emoji") ?: "", it.getString("label") ?: "") },
                menu = tables("menu").map { MenuItem(it.getString("label") ?: "", it.getString("command") ?: "") },
                commands = tables("commands").map { CommandSpec(it.getString("name") ?: "", it.getString("description") ?: "") },
            )
        }
    }
}

/** The bundled profiles, by id. */
object Harnesses {
    @Volatile private var all: Map<String, Harness> = mapOf("generic" to Harness())

    fun load(context: Context) {
        val files = context.assets.list("agents").orEmpty().filter { it.endsWith(".toml") }
        val loaded = files.mapNotNull { f ->
            runCatching { Harness.parse(context.assets.open("agents/$f").bufferedReader().readText()) }.getOrNull()
        }
        if (loaded.isNotEmpty()) all = loaded.associateBy { it.id }
    }

    fun get(id: String?): Harness = all[id ?: Harness.DEFAULT_ID] ?: all[Harness.DEFAULT_ID] ?: all.values.first()

    fun list(): List<Harness> = all.values.sortedBy { it.name }

    /** For tests. */
    fun set(harnesses: List<Harness>) {
        all = harnesses.associateBy { it.id }
    }
}
