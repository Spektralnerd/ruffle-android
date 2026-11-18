package rs.ruffle

import android.content.Context
import android.net.Uri
import org.json.JSONArray
import org.json.JSONException
import org.json.JSONObject

/** A movie opened from the launcher. */
data class RecentFile(
    val uri: String,
    val name: String,
    val favorite: Boolean,
    val lastOpened: Long
)

/**
 * The movies opened from the launcher (history), and the ones marked as favorites.
 * Favorites are kept until they are removed, the rest only the last [MAX_HISTORY] ones.
 */
class RecentFiles(context: Context) {
    private val prefs = context.getSharedPreferences("recent_files", Context.MODE_PRIVATE)

    /** Favorites first, then the most recently opened. */
    fun load(): List<RecentFile> {
        val json = prefs.getString(KEY, null) ?: return emptyList()
        return try {
            val array = JSONArray(json)
            (0 until array.length()).map { i ->
                val item = array.getJSONObject(i)
                RecentFile(
                    uri = item.getString("uri"),
                    name = item.getString("name"),
                    favorite = item.optBoolean("favorite"),
                    lastOpened = item.optLong("lastOpened")
                )
            }.sortedWith(
                compareByDescending<RecentFile> { it.favorite }.thenByDescending { it.lastOpened }
            )
        } catch (e: JSONException) {
            emptyList()
        }
    }

    fun add(uri: Uri, name: String) {
        val now = System.currentTimeMillis()
        val items = load()
        val existing = items.find { it.uri == uri.toString() }
        val others = items.filter { it.uri != uri.toString() }
        val updated = RecentFile(uri.toString(), name, existing?.favorite ?: false, now)
        val history = others.filter { !it.favorite }.take(MAX_HISTORY - 1)
        save(listOf(updated) + others.filter { it.favorite } + history)
    }

    fun setFavorite(uri: String, favorite: Boolean) {
        save(load().map { if (it.uri == uri) it.copy(favorite = favorite) else it })
    }

    fun remove(uri: String) {
        save(load().filter { it.uri != uri })
    }

    /** Removes everything but the favorites. */
    fun clearHistory() {
        save(load().filter { it.favorite })
    }

    private fun save(items: List<RecentFile>) {
        val array = JSONArray()
        for (item in items) {
            array.put(
                JSONObject()
                    .put("uri", item.uri)
                    .put("name", item.name)
                    .put("favorite", item.favorite)
                    .put("lastOpened", item.lastOpened)
            )
        }
        prefs.edit().putString(KEY, array.toString()).apply()
    }

    private companion object {
        const val KEY = "items"
        const val MAX_HISTORY = 20
    }
}
