package rs.ruffle

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.SystemClock
import android.view.KeyEvent
import androidx.test.core.app.ApplicationProvider
import androidx.test.espresso.matcher.ViewMatchers.assertThat
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import java.io.File
import org.hamcrest.CoreMatchers.equalTo
import org.hamcrest.CoreMatchers.notNullValue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

private const val PACKAGE = "rs.ruffle"
private const val TIMEOUT = 10000L

/** Tests of the player's loading indicator, error handling and gamepad support. */
@RunWith(AndroidJUnit4::class)
class PlayerFeatures {
    private lateinit var device: UiDevice
    private lateinit var context: Context
    private lateinit var traceOutput: File

    @Before
    fun setUp() {
        device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        context = ApplicationProvider.getApplicationContext()
        traceOutput = File.createTempFile("trace", ".txt", context.cacheDir)
        device.pressHome()
    }

    private fun startPlayer(movie: ByteArray) {
        val swfFile = File.createTempFile("movie", ".swf", context.cacheDir)
        swfFile.writeBytes(movie)
        val intent = Intent().apply {
            component = ComponentName(PACKAGE, "rs.ruffle.PlayerActivity")
            data = Uri.fromFile(swfFile)
            putExtra("traceOutput", traceOutput.absolutePath)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK)
        }
        context.startActivity(intent)
        device.wait(Until.hasObject(By.pkg(PACKAGE).depth(0)), TIMEOUT)
    }

    private fun testMovie(id: Int): ByteArray =
        InstrumentationRegistry.getInstrumentation().context.resources.openRawResource(id)
            .use { it.readBytes() }

    /** Waits until the trace has [count] lines, and returns them. */
    private fun waitForTrace(count: Int): List<String> {
        val timeoutAt = SystemClock.uptimeMillis() + TIMEOUT
        var lines = traceOutput.readLines()
        while (lines.size < count && SystemClock.uptimeMillis() < timeoutAt) {
            Thread.sleep(100)
            lines = traceOutput.readLines()
        }
        return lines
    }

    @Test
    fun loadingIndicatorDisappears() {
        startPlayer(testMovie(rs.ruffle.test.R.raw.helloflash))
        assertThat(waitForTrace(1), equalTo(listOf("Hello from Flash!")))
        assertThat(
            device.wait(Until.gone(By.res(PACKAGE, "loading")), TIMEOUT),
            equalTo(true)
        )
    }

    @Test
    fun invalidMovieShowsError() {
        startPlayer("This is not a SWF file".toByteArray())
        val dialog = device.wait(Until.findObject(By.text("Not a Flash movie")), TIMEOUT)
        assertThat(dialog, notNullValue())
    }

    @Test
    fun gamepadButtonsArePressedAsKeys() {
        startPlayer(testMovie(rs.ruffle.test.R.raw.input_test))
        assertThat(waitForTrace(1), equalTo(listOf("Test started!")))

        // A is mapped to Space, Start to Enter.
        device.pressKeyCode(KeyEvent.KEYCODE_BUTTON_A)
        device.pressKeyCode(KeyEvent.KEYCODE_BUTTON_START)

        assertThat(
            waitForTrace(5),
            equalTo(
                listOf(
                    "Test started!",
                    "keyDown: keyCode = 32, charCode = 0",
                    "keyUp: keyCode = 32, charCode = 0",
                    "keyDown: keyCode = 13, charCode = 0",
                    "keyUp: keyCode = 13, charCode = 0"
                )
            )
        )
    }
}
