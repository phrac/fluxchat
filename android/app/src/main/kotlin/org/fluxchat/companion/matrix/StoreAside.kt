package org.fluxchat.companion.matrix

import java.io.File

/**
 * A new sign-in gets a new store — but the old one is only deleted once the
 * new sign-in has actually authenticated (the desktop's
 * `store_migration::set_aside` / `put_back` / `drop_aside`). Until then it
 * waits next to the store as `<store>.previous`, so a leftover one always
 * means "put it back": a wrong password, no network, or the process dying
 * mid-sign-in must not cost the device its keys.
 *
 * [cache] holds the store's rebuildable half (event cache, media); it is
 * encrypted with the store's passphrase, so it never outlives a swap.
 */
class StoreAside(private val store: File, private val cache: File) {
    private val aside = File(store.parentFile, store.name + ".previous")
    /** Where a store goes to be deleted: `.previous` must vanish in one step (see [drop]). */
    private val discard = File(store.parentFile, store.name + ".discard")

    /** Before a sign-in: move the current store out of the way. */
    fun setAside() {
        if (store.exists()) {
            // An older one already waiting is the real one: keep it.
            if (aside.exists() || !store.renameTo(aside)) store.deleteRecursively()
        }
        cache.deleteRecursively()
    }

    /** The sign-in failed (or never finished): the old store comes back. */
    fun putBack() {
        discard.deleteRecursively()
        if (!aside.exists()) return
        store.deleteRecursively()
        aside.renameTo(store)
        cache.deleteRecursively()
    }

    /**
     * The sign-in authenticated: the old store is no longer anyone's. Renamed
     * away first, then deleted — a half-deleted `.previous` would be put back
     * over the new store at the next start.
     */
    fun drop() {
        discard.deleteRecursively()
        if (aside.exists() && !aside.renameTo(discard)) aside.deleteRecursively()
        discard.deleteRecursively()
    }
}
