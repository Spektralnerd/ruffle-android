package rs.ruffle

import android.content.ActivityNotFoundException
import android.content.res.Configuration
import android.net.Uri
import android.util.Log
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.paddingFromBaseline
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.wrapContentSize
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import rs.ruffle.ui.theme.RuffleTheme
import rs.ruffle.ui.theme.SLIGHTLY_DEEMPHASIZED_ALPHA

@Composable
fun BrandBar() {
    Image(
        painter = painterResource(id = R.drawable.ic_logo_dark),
        contentDescription = stringResource(id = R.string.logo_description),
        modifier = Modifier
            .wrapContentSize(align = Alignment.Center)
            .padding(vertical = 75.dp)
    )
}

@Composable
fun SelectSwfRoute(recentFiles: RecentFiles, openSwf: (uri: Uri) -> Unit) {
    var recentItems by remember { mutableStateOf(recentFiles.load()) }
    SelectSwfScreen(
        openSwf = { uri ->
            openSwf(uri)
            recentItems = recentFiles.load()
        },
        recentItems = recentItems,
        onToggleFavorite = { item ->
            recentFiles.setFavorite(item.uri, !item.favorite)
            recentItems = recentFiles.load()
        },
        onRemove = { item ->
            recentFiles.remove(item.uri)
            recentItems = recentFiles.load()
        },
        onClearHistory = {
            recentFiles.clearHistory()
            recentItems = recentFiles.load()
        }
    )
}

@Composable
fun SelectSwfScreen(
    openSwf: (uri: Uri) -> Unit,
    recentItems: List<RecentFile> = emptyList(),
    onToggleFavorite: (RecentFile) -> Unit = {},
    onRemove: (RecentFile) -> Unit = {},
    onClearHistory: () -> Unit = {}
) {
    Scaffold { innerPadding ->
        BoxWithConstraints(
            modifier = Modifier
                .padding(innerPadding)
                .fillMaxSize()
        ) {
            // Centered while everything fits on the screen, scrollable when the list is long.
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState())
                    .heightIn(min = maxHeight),
                verticalArrangement = Arrangement.Center
            ) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .wrapContentSize(align = Alignment.Center)
                ) {
                    BrandBar()
                }
                Text(
                    modifier = Modifier
                        .align(Alignment.CenterHorizontally)
                        .wrapContentSize(align = Alignment.Center)
                        .padding(horizontal = 8.dp, vertical = 20.dp),
                    text = stringResource(id = R.string.work_in_progress_warning),
                    textAlign = TextAlign.Center
                )
                SelectSwfUrlOrFile(openSwf)
                if (recentItems.isNotEmpty()) {
                    RecentFilesList(
                        items = recentItems,
                        onOpen = { openSwf(Uri.parse(it.uri)) },
                        onToggleFavorite = onToggleFavorite,
                        onRemove = onRemove,
                        onClearHistory = onClearHistory
                    )
                }
            }
        }
    }
}

@Composable
private fun RecentFilesList(
    items: List<RecentFile>,
    onOpen: (RecentFile) -> Unit,
    onToggleFavorite: (RecentFile) -> Unit,
    onRemove: (RecentFile) -> Unit,
    onClearHistory: () -> Unit
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .wrapContentSize(align = Alignment.TopCenter)
            .widthIn(max = 488.dp)
            .padding(horizontal = 12.dp, vertical = 8.dp)
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = stringResource(id = R.string.recent),
                style = MaterialTheme.typography.titleMedium,
                modifier = Modifier
                    .weight(1f)
                    .padding(start = 8.dp)
            )
            if (items.any { !it.favorite }) {
                TextButton(onClick = onClearHistory) {
                    Text(text = stringResource(id = R.string.clear_history))
                }
            }
        }
        for (item in items) {
            RecentFileRow(
                item = item,
                onOpen = { onOpen(item) },
                onToggleFavorite = { onToggleFavorite(item) },
                onRemove = { onRemove(item) }
            )
        }
    }
}

@Composable
private fun RecentFileRow(
    item: RecentFile,
    onOpen: () -> Unit,
    onToggleFavorite: () -> Unit,
    onRemove: () -> Unit
) {
    val favoriteDescription = stringResource(
        id = if (item.favorite) R.string.remove_favorite else R.string.add_favorite
    )
    val removeDescription = stringResource(id = R.string.remove_from_list)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onOpen),
        verticalAlignment = Alignment.CenterVertically
    ) {
        IconButton(
            onClick = onToggleFavorite,
            modifier = Modifier.semantics { contentDescription = favoriteDescription }
        ) {
            Text(
                text = if (item.favorite) "★" else "☆",
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.primary
            )
        }
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = item.name,
                style = MaterialTheme.typography.bodyLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
            val host = Uri.parse(item.uri).takeIf { it.scheme != "content" }?.host
            if (!host.isNullOrEmpty()) {
                Text(
                    text = host,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurface.copy(
                        alpha = SLIGHTLY_DEEMPHASIZED_ALPHA
                    ),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
            }
        }
        IconButton(
            onClick = onRemove,
            modifier = Modifier.semantics { contentDescription = removeDescription }
        ) {
            Text(text = "✕", style = MaterialTheme.typography.titleMedium)
        }
    }
}

@Composable
private fun SelectSwfUrlOrFile(openSwf: (uri: Uri) -> Unit) {
    val urlState by rememberSaveable(stateSaver = UrlStateSaver) {
        mutableStateOf(UrlState())
    }
    Column(
        modifier = Modifier
            .padding(horizontal = 20.dp)
    ) {
        val submitUrl = {
            if (urlState.isValid) {
                openSwf(Uri.parse(urlState.text))
            } else {
                urlState.enableShowErrors()
            }
        }
        OutlinedTextField(
            value = urlState.text,
            onValueChange = { urlState.text = it },
            modifier = Modifier
                .fillMaxWidth()
                .wrapContentSize(align = Alignment.Center)
                .width(488.dp)
                .onFocusChanged { focusState ->
                    urlState.onFocusChange(focusState.isFocused)
                    if (!focusState.isFocused) {
                        urlState.enableShowErrors()
                    }
                },
            label = {
                Text(
                    text = stringResource(id = R.string.url),
                    style = MaterialTheme.typography.bodyMedium
                )
            },
            textStyle = MaterialTheme.typography.bodyMedium,
            isError = urlState.showErrors(),
            keyboardOptions = KeyboardOptions.Default.copy(
                imeAction = ImeAction.Go,
                keyboardType = KeyboardType.Uri
            ),
            keyboardActions = KeyboardActions(
                onDone = { submitUrl() }
            ),
            singleLine = true
        )
        urlState.getError()?.let { error -> TextFieldError(textError = error) }

        Button(
            onClick = submitUrl,
            modifier = Modifier
                .fillMaxWidth()
                .wrapContentSize(align = Alignment.Center)
                .width(320.dp)
                .padding(top = 28.dp, bottom = 3.dp)
        ) {
            Text(
                text = stringResource(id = R.string.open_url),
                style = MaterialTheme.typography.titleSmall
            )
        }

        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier = Modifier
                .fillMaxWidth()
                .wrapContentSize(align = Alignment.Center)
                .width(320.dp)
        ) {
            Text(
                text = stringResource(id = R.string.or),
                style = MaterialTheme.typography.titleSmall,
                color = MaterialTheme.colorScheme.onSurface.copy(
                    alpha = SLIGHTLY_DEEMPHASIZED_ALPHA
                ),
                modifier = Modifier.paddingFromBaseline(top = 25.dp)
            )
            PickSwfButton(openSwf)
        }
    }
}

@Composable
fun PickSwfButton(onSelect: (uri: Uri) -> Unit) {
    // The document picker (rather than GetContent) can grant a lasting permission to read the
    // file, so that it can be opened again from the list of recent files.
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) {
        if (it != null) {
            onSelect(it)
        }
    }
    // Some devices (e.g. TV boxes) have no document picker, only file managers.
    val fallbackLauncher =
        rememberLauncherForActivityResult(ActivityResultContracts.GetContent()) {
            if (it != null) {
                onSelect(it)
            }
        }

    OutlinedButton(
        onClick = {
            // Ideally "application/x-shockwave-flash" would be
            // used here, but Android doesn't recognize many
            // downloaded .swf files as such for some reason... :/
            try {
                launcher.launch(arrayOf("*/*"))
            } catch (e: ActivityNotFoundException) {
                try {
                    fallbackLauncher.launch("*/*")
                } catch (fallbackNotFound: ActivityNotFoundException) {
                    Log.w("ruffle", "No app to pick files with", fallbackNotFound)
                }
            }
        },
        modifier = Modifier
            .width(320.dp)
            .padding(top = 20.dp, bottom = 24.dp)
            .fillMaxWidth()
            .wrapContentSize(align = Alignment.Center)
            .width(320.dp)

    ) {
        Text(text = stringResource(id = R.string.select_a_swf))
    }
}

@Composable
fun TextFieldError(textError: String) {
    Row(modifier = Modifier.fillMaxWidth()) {
        Spacer(modifier = Modifier.width(16.dp))
        Text(
            text = textError,
            modifier = Modifier.fillMaxWidth(),
            color = MaterialTheme.colorScheme.error
        )
    }
}

@Preview(name = "Select SWF - Light", uiMode = Configuration.UI_MODE_NIGHT_NO)
@Preview(name = "Select SWF - Dark", uiMode = Configuration.UI_MODE_NIGHT_YES)
@Composable
fun SelectSwfScreenPreview() {
    RuffleTheme {
        Surface {
            SelectSwfScreen(
                openSwf = {},
                recentItems = listOf(
                    RecentFile("content://example/1", "favorite-game.swf", true, 2),
                    RecentFile("https://example.com/movie.swf", "movie.swf", false, 1)
                )
            )
        }
    }
}
