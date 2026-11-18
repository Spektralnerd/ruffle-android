package rs.ruffle

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.provider.OpenableColumns
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import rs.ruffle.ui.theme.RuffleTheme

class MainActivity : ComponentActivity() {
    private lateinit var recentFiles: RecentFiles

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        recentFiles = RecentFiles(this)

        setContent {
            RuffleTheme {
                // (Only one screen, so no navigation library: its current version needs
                // Android 7.)
                SelectSwfRoute(recentFiles = recentFiles, openSwf = { openSwf(it) })
            }
        }
    }

    private fun openSwf(uri: Uri) {
        if (uri.scheme == "content") {
            // Keep the permission to read the file, so it can be opened again from the list
            // of recent files later (only possible for files picked with the document picker).
            try {
                contentResolver.takePersistableUriPermission(
                    uri,
                    Intent.FLAG_GRANT_READ_URI_PERMISSION
                )
            } catch (e: SecurityException) {
                Log.i("ruffle", "No lasting permission to read $uri", e)
            }
        }
        recentFiles.add(uri, displayName(uri))

        val intent = Intent(
            this@MainActivity,
            PlayerActivity::class.java
        ).apply {
            data = uri
        }
        startActivity(intent)
    }

    /** A name to show for a movie: its file name, or the last part of its URL. */
    private fun displayName(uri: Uri): String {
        if (uri.scheme == "content") {
            try {
                contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
                    ?.use { cursor ->
                        if (cursor.moveToFirst()) {
                            cursor.getString(0)?.let { return it }
                        }
                    }
            } catch (e: Exception) {
                Log.w("ruffle", "Couldn't get the name of $uri", e)
            }
        }
        return uri.lastPathSegment?.takeIf { it.isNotBlank() } ?: uri.toString()
    }
}
