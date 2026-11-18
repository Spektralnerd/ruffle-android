package rs.ruffle

import android.annotation.SuppressLint
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.res.Configuration
import android.net.Uri
import android.os.Build
import android.os.Build.VERSION_CODES
import android.os.Bundle
import android.provider.OpenableColumns
import android.util.Log
import android.view.KeyEvent
import android.view.Menu
import android.view.MenuItem
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.view.Window
import android.view.WindowManager
import android.widget.Button
import android.widget.PopupMenu
import androidx.appcompat.app.AlertDialog
import androidx.constraintlayout.widget.ConstraintLayout
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import com.google.androidgamesdk.GameActivity
import java.io.File

class PlayerActivity : GameActivity() {
    private var clipboard: ClipboardManager? = null

    /** An error dialog is (or was) shown, after which the player closes. */
    private var errorShown = false

    @Suppress("unused")
    // Used by Rust
    private val swfBytes: ByteArray?
        get() {
            val uri = intent.data
            if (uri?.scheme == "content") {
                try {
                    // (InputStream.available() is not the size of the file.)
                    return contentResolver.openInputStream(uri)?.use { it.readBytes() }
                } catch (e: Exception) {
                    // E.g. the permission to read it is gone. Rust then reports the error.
                    Log.w("ruffle", "Couldn't read $uri", e)
                }
            }
            return null
        }

    @Suppress("unused")
    // Used by Rust
    private val swfUri: String?
        get() {
            return intent.dataString
        }

    /** The file name of a movie opened from a `content://` URI, if there is one. */
    @Suppress("unused")
    // Used by Rust
    private fun getSwfDisplayName(): String? {
        val uri = intent.data ?: return null
        if (uri.scheme != "content") {
            return null
        }
        return try {
            contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
                ?.use { cursor -> if (cursor.moveToFirst()) cursor.getString(0) else null }
        } catch (e: Exception) {
            Log.w("ruffle", "Couldn't get the name of $uri", e)
            null
        }
    }

    @Suppress("unused")
    // Used by Rust
    private val traceOutput: String?
        get() {
            return intent.getStringExtra("traceOutput")
        }

    @Suppress("unused")
    // Used by Rust
    private fun navigateToUrl(url: String?) {
        startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
    }

    private var loc = IntArray(2)

    @Suppress("unused")
    // Handle of an EventLoopProxy over in rust-land
    private val eventLoopHandle: Long = 0

    @Suppress("unused")
    // Used by Rust
    private val locInWindow: IntArray
        get() {
            mSurfaceView.getLocationInWindow(loc)
            return loc
        }

    @Suppress("unused")
    // Used by Rust
    private val surfaceWidth: Int
        get() = mSurfaceView.width

    @Suppress("unused")
    // Used by Rust
    private val surfaceHeight: Int
        get() = mSurfaceView.height

    private external fun keydown(keyTag: String)
    private external fun keyup(keyTag: String)
    private external fun requestContextMenu()
    private external fun runContextMenuCallback(index: Int)
    private external fun clearContextMenu()
    private external fun showSoftKeyboard()

    @Suppress("unused")
    // Used by Rust
    private fun showContextMenu(items: Array<String>) {
        runOnUiThread {
            val popup = PopupMenu(this, findViewById(R.id.button_cm))
            val menu = popup.menu
            if (Build.VERSION.SDK_INT >= VERSION_CODES.P) {
                menu.setGroupDividerEnabled(true)
            }
            var group = 1
            for (i in items.indices) {
                val elements = items[i].split(" ".toRegex(), limit = 4).toTypedArray()
                val enabled = elements[0].toBoolean()
                val separatorBefore = elements[1].toBoolean()
                val checked = elements[2].toBoolean()
                val caption = elements[3]
                if (separatorBefore) group += 1
                val item = menu.add(group, i, Menu.NONE, caption)
                item.setEnabled(enabled)
                if (checked) {
                    item.setCheckable(true)
                    item.setChecked(true)
                }
            }
            val exitItemId: Int = items.size
            menu.add(group, exitItemId, Menu.NONE, R.string.exit)
            popup.setOnMenuItemClickListener { item: MenuItem ->
                if (item.itemId == exitItemId) {
                    finish()
                } else {
                    runContextMenuCallback(item.itemId)
                }
                true
            }
            popup.setOnDismissListener { clearContextMenu() }
            popup.show()
        }
    }

    @Suppress("unused")
    // Used by Rust
    private fun getAndroidDataStorageDir(): String {
        // TODO It can also be placed in an external storage path in the future to share archived content
        val storageDirPath = "${filesDir.absolutePath}/ruffle/shared_objects"
        val storageDir = File(storageDirPath)
        if (!storageDir.exists()) {
            storageDir.mkdirs()
        }
        return storageDirPath
    }

    /** The movie is loaded, so the loading indicator can go. */
    @Suppress("unused")
    // Used by Rust
    private fun onMovieLoaded() {
        runOnUiThread { findViewById<View>(R.id.loading)?.visibility = View.GONE }
    }

    /**
     * Explains why the movie can't be played, and closes the player afterwards.
     * [kind] is one of the `ERROR_` constants, [details] the technical error message.
     */
    @Suppress("unused")
    // Used by Rust
    private fun showError(kind: Int, details: String) {
        runOnUiThread {
            if (errorShown || isFinishing) {
                return@runOnUiThread
            }
            errorShown = true
            findViewById<View>(R.id.loading)?.visibility = View.GONE
            val title = when (kind) {
                ERROR_INVALID_MOVIE -> R.string.error_invalid_movie_title
                ERROR_RENDERER_FAILED -> R.string.error_renderer_title
                else -> R.string.error_load_failed_title
            }
            val message = when (kind) {
                ERROR_INVALID_MOVIE -> R.string.error_invalid_movie
                ERROR_RENDERER_FAILED -> R.string.error_renderer
                else -> R.string.error_load_failed
            }
            AlertDialog.Builder(this)
                .setTitle(title)
                .setMessage(getString(message, details))
                .setPositiveButton(R.string.close) { _, _ -> finish() }
                .setOnCancelListener { finish() }
                .show()
        }
    }

    /** A message from the movie. */
    @Suppress("unused")
    // Used by Rust
    private fun showMessage(message: String) {
        runOnUiThread {
            if (!isFinishing) {
                AlertDialog.Builder(this)
                    .setMessage(message)
                    .setPositiveButton(android.R.string.ok, null)
                    .show()
            }
        }
    }

    @Suppress("unused")
    // Used by Rust
    private fun getClipboardText(): String = try {
        val clip = clipboard?.primaryClip
        if (clip != null && clip.itemCount > 0) {
            clip.getItemAt(0).coerceToText(this).toString()
        } else {
            ""
        }
    } catch (e: Exception) {
        Log.w("ruffle", "Couldn't read the clipboard", e)
        ""
    }

    @Suppress("unused")
    // Used by Rust
    private fun setClipboardText(text: String) {
        try {
            clipboard?.setPrimaryClip(ClipData.newPlainText(getString(R.string.app_name), text))
        } catch (e: Exception) {
            Log.w("ruffle", "Couldn't write to the clipboard", e)
        }
    }

    override fun onCreateSurfaceView() {
        val inflater = layoutInflater

        @SuppressLint("InflateParams")
        val layout = inflater.inflate(R.layout.keyboard, null) as ConstraintLayout

        contentViewId = View.generateViewId()
        layout.id = contentViewId
        setContentView(layout)
        mSurfaceView = InputEnabledSurfaceView(this)

        mSurfaceView.contentDescription = "Ruffle Player"

        val placeholder = findViewById<View>(R.id.placeholder)
        val pars = placeholder.layoutParams as ConstraintLayout.LayoutParams
        val parent = placeholder.parent as ViewGroup
        val index = parent.indexOfChild(placeholder)
        parent.removeView(placeholder)
        parent.addView(mSurfaceView, index)
        mSurfaceView.setLayoutParams(pars)
        val keys = gatherAllDescendantsOfType<Button>(
            layout.getViewById(R.id.keyboard),
            Button::class.java
        )
        for (b in keys) {
            b.setOnTouchListener { view: View, motionEvent: MotionEvent ->
                val tag = view.tag as String
                if (motionEvent.action == MotionEvent.ACTION_DOWN) keydown(tag)
                if (motionEvent.action == MotionEvent.ACTION_UP) keyup(tag)
                view.performClick()
                false
            }
        }
        val keyboard = layout.getViewById(R.id.keyboard)
        keyboard.visibility = if (isLandscape(resources.configuration)) View.GONE else View.VISIBLE
        layout.findViewById<View>(R.id.button_kb).setOnClickListener {
            if (keyboard.visibility == View.VISIBLE) {
                keyboard.visibility = View.GONE
            } else {
                keyboard.visibility = View.VISIBLE
            }
        }
        layout.findViewById<View>(R.id.button_ime).setOnClickListener { showSoftKeyboard() }
        layout.findViewById<View>(R.id.button_cm)
            .setOnClickListener { requestContextMenu() }
        layout.requestLayout()
        layout.requestFocus()
        mSurfaceView.holder.addCallback(this)
        ViewCompat.setOnApplyWindowInsetsListener(mSurfaceView, this)
        // Keep the movie and the buttons away from display cutouts (notches, camera holes),
        // and above the soft keyboard. The insets are passed on unchanged, so that the
        // surface view's own listener (GameActivity's) still gets them.
        ViewCompat.setOnApplyWindowInsetsListener(layout) { view, insets ->
            val cutout = insets.getInsets(WindowInsetsCompat.Type.displayCutout())
            val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
            view.setPadding(
                cutout.left,
                cutout.top,
                cutout.right,
                maxOf(cutout.bottom, ime.bottom)
            )
            insets
        }
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        val keyboard = findViewById<View>(R.id.keyboard)
        keyboard.visibility = if (isLandscape(newConfig)) View.GONE else View.VISIBLE
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) {
            // Dialogs, menus and the soft keyboard can bring the system bars back.
            hideSystemUI()
        }
    }

    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        when (event.keyCode) {
            // GameActivity passes Back to the movie, which doesn't use it, so the player
            // couldn't be left with it.
            KeyEvent.KEYCODE_BACK -> {
                if (event.action == KeyEvent.ACTION_UP && !event.isCanceled) {
                    finish()
                }
                return true
            }
            // While the soft keyboard is active, GameTextInput handles these keys using an
            // API 24 method, which crashes the app on older versions. Give them straight to
            // the player there; it deletes the text in the movie by itself.
            KeyEvent.KEYCODE_DEL, KeyEvent.KEYCODE_FORWARD_DEL -> {
                if (Build.VERSION.SDK_INT < VERSION_CODES.N) {
                    return when (event.action) {
                        KeyEvent.ACTION_DOWN -> onKeyDown(event.keyCode, event)
                        KeyEvent.ACTION_UP -> onKeyUp(event.keyCode, event)
                        else -> true
                    }
                }
            }
        }
        return super.dispatchKeyEvent(event)
    }

    private fun hideSystemUI() {
        // This will put the game behind any cutouts and waterfalls on devices which have
        // them, so the corresponding insets will be non-zero.
        if (Build.VERSION.SDK_INT >= VERSION_CODES.R) {
            window.attributes.layoutInDisplayCutoutMode =
                WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS
        } else if (Build.VERSION.SDK_INT >= VERSION_CODES.P) {
            window.attributes.layoutInDisplayCutoutMode =
                WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
        }
        // From API 30 onwards, this is the recommended way to hide the system UI, rather than
        // using View.setSystemUiVisibility.
        val decorView = window.decorView
        val controller = WindowInsetsControllerCompat(
            window,
            decorView
        )
        controller.hide(WindowInsetsCompat.Type.systemBars())
        controller.hide(WindowInsetsCompat.Type.displayCutout())
        controller.systemBarsBehavior =
            WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        nativeInit { message ->
            Log.e("ruffle", "Handling panic: $message")
            startActivity(
                Intent(this, PanicActivity::class.java).apply {
                    putExtra("message", message)
                }
            )
            // The player can't continue after this.
            runOnUiThread { finish() }
        }
        clipboard = getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
        // When true, the app will fit inside any system UI windows.
        // When false, we render behind any system UI windows.
        WindowCompat.setDecorFitsSystemWindows(window, false)
        hideSystemUI()
        // You can set IME fields here or in native code using GameActivity_setImeEditorInfoFields.
        // We set the fields in native_engine.cpp.
        // super.setImeEditorInfoFields(InputType.TYPE_CLASS_TEXT,
        //     IME_ACTION_NONE, IME_FLAG_NO_FULLSCREEN );
        requestNoStatusBarFeature()
        supportActionBar?.hide()
        super.onCreate(savedInstanceState)
    }

    // Used by Rust
    @Suppress("unused")
    val isGooglePlayGames: Boolean
        get() {
            val pm = packageManager
            return pm.hasSystemFeature("com.google.android.play.feature.HPE_EXPERIENCE")
        }

    private fun requestNoStatusBarFeature() {
        // Hiding the status bar this way makes it see through when pulled down
        requestWindowFeature(Window.FEATURE_NO_TITLE)
        WindowInsetsControllerCompat(
            window,
            mSurfaceView
        ).hide(WindowInsetsCompat.Type.statusBars())
    }

    companion object {
        // Error kinds for showError(), see `ErrorKind` in src/ui.rs.
        private const val ERROR_INVALID_MOVIE = 1
        private const val ERROR_RENDERER_FAILED = 2

        init {
            // load the native activity
            System.loadLibrary("ruffle_android")
        }

        @JvmStatic
        private external fun nativeInit(crashCallback: CrashCallback)

        private fun isLandscape(configuration: Configuration) =
            configuration.orientation == Configuration.ORIENTATION_LANDSCAPE

        private fun <T> gatherAllDescendantsOfType(v: View, t: Class<*>): List<T> {
            val result: MutableList<T> = ArrayList()
            @Suppress("UNCHECKED_CAST")
            if (t.isInstance(v)) result.add(v as T)
            if (v is ViewGroup) {
                for (i in 0 until v.childCount) {
                    result.addAll(gatherAllDescendantsOfType(v.getChildAt(i), t))
                }
            }
            return result
        }
    }

    fun interface CrashCallback {
        fun onCrash(message: String)
    }
}
