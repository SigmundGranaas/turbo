package com.sigmundgranaas.turbo.expressive.feature.search

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.History
import androidx.compose.material.icons.rounded.NorthWest
import androidx.compose.material.icons.rounded.Place
import androidx.compose.material.icons.rounded.Public
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.runtime.LaunchedEffect
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sigmundgranaas.turbo.expressive.ui.components.Cookie
import com.sigmundgranaas.turbo.expressive.ui.layout.responsiveContentWidth
import com.sigmundgranaas.turbo.expressive.ui.components.EmptyState
import com.sigmundgranaas.turbo.expressive.ui.components.ErrorState
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius
import com.sigmundgranaas.turbo.expressive.ui.theme.icon

/**
 * Bold only the leading [query] when [name] actually starts with it (case-insensitive);
 * otherwise render the name plain. Avoids the "StorSjurfjellet" artifact where a
 * non-matching name had the query blindly prepended in bold.
 */
internal fun highlightPrefix(name: String, query: String): AnnotatedString = buildAnnotatedString {
    val q = query.trim()
    if (q.isNotEmpty() && name.startsWith(q, ignoreCase = true)) {
        withStyle(SpanStyle(fontWeight = FontWeight.W800)) { append(name.substring(0, q.length)) }
        append(name.substring(q.length))
    } else {
        append(name)
    }
}

@Composable
fun SearchScreen(
    onBack: () -> Unit,
    onPick: (lat: Double, lng: Double, name: String) -> Unit,
    viewModel: SearchViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val ui by viewModel.state.collectAsStateWithLifecycle()
    val recents by viewModel.recents.collectAsStateWithLifecycle()
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    val pick: (SearchResult) -> Unit = { r ->
        if (r.lat != null && r.lng != null) {
            viewModel.recordPick(r)
            onPick(r.lat, r.lng, r.name)
        }
    }

    Column(Modifier.fillMaxSize().background(cs.surface).statusBarsPadding().imePadding()) {
        Surface(
            shape = CircleShape, color = cs.surfaceContainerHigh, shadowElevation = 1.dp,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 14.dp).fillMaxWidth().height(56.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(horizontal = 8.dp)) {
                IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.search_back), tint = cs.onSurface) }
                Box(Modifier.weight(1f).padding(horizontal = 4.dp), contentAlignment = Alignment.CenterStart) {
                    if (ui.query.isEmpty()) {
                        Text(stringResource(R.string.search_hint), style = MaterialTheme.typography.bodyLarge, color = cs.onSurfaceVariant)
                    }
                    BasicTextField(
                        value = ui.query,
                        onValueChange = viewModel::setQuery,
                        singleLine = true,
                        textStyle = MaterialTheme.typography.bodyLarge.copy(color = cs.onSurface),
                        cursorBrush = SolidColor(cs.primary),
                        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                        modifier = Modifier.fillMaxWidth().focusRequester(focus),
                    )
                }
                if (ui.query.isNotEmpty()) {
                    IconButton(onClick = { viewModel.setQuery("") }) { Icon(Icons.Rounded.Close, stringResource(R.string.search_clear), tint = cs.onSurfaceVariant) }
                }
            }
        }

        Row(Modifier.padding(horizontal = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FilterChip(selected = ui.filter == 0, onClick = { viewModel.setFilter(0) }, label = { Text(stringResource(R.string.search_filter_all)) })
            FilterChip(selected = ui.filter == 1, onClick = { viewModel.setFilter(1) }, label = { Text(stringResource(R.string.search_filter_markers)) }, leadingIcon = { Icon(Icons.Rounded.Place, null, Modifier.size(18.dp)) })
            FilterChip(selected = ui.filter == 2, onClick = { viewModel.setFilter(2) }, label = { Text(stringResource(R.string.search_filter_places)) }, leadingIcon = { Icon(Icons.Rounded.Public, null, Modifier.size(18.dp)) })
        }

        when {
            ui.loading -> Box(Modifier.fillMaxWidth().padding(top = 48.dp), contentAlignment = Alignment.Center) {
                CircularProgressIndicator()
            }
            ui.query.isBlank() && recents.isNotEmpty() -> LazyColumn(Modifier.fillMaxHeight().responsiveContentWidth().padding(horizontal = 8.dp, vertical = 4.dp)) {
                item {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.fillMaxWidth().padding(start = 12.dp, end = 4.dp, top = 8.dp, bottom = 4.dp),
                    ) {
                        Text(stringResource(R.string.search_recent), style = MaterialTheme.typography.titleSmall, color = cs.onSurfaceVariant, modifier = Modifier.weight(1f))
                        Text(
                            stringResource(R.string.search_clear),
                            style = MaterialTheme.typography.labelLarge,
                            color = cs.primary,
                            modifier = Modifier.clip(RoundedCornerShape(TurboRadius.m)).clickable { viewModel.clearRecents() }.padding(horizontal = 10.dp, vertical = 6.dp),
                        )
                    }
                }
                items(recents.size) { i ->
                    val rs = recents[i]
                    RecentRow(rs) { pick(SearchResult(rs.name, rs.sub, com.sigmundgranaas.turbo.expressive.domain.ActivityKindId.Mountain, SearchResultType.Place, rs.lat, rs.lng)) }
                }
            }
            ui.query.isBlank() -> EmptyState(
                icon = Icons.Rounded.Search,
                title = stringResource(R.string.search_empty_title),
                body = stringResource(R.string.search_empty_body),
                modifier = Modifier.fillMaxSize(),
            )
            ui.error && ui.results.isEmpty() -> ErrorState(
                message = stringResource(R.string.search_error),
                onRetry = viewModel::retry,
                modifier = Modifier.fillMaxSize(),
            )
            ui.results.isEmpty() -> EmptyState(
                icon = Icons.Rounded.Search,
                title = stringResource(R.string.search_no_matches),
                body = stringResource(R.string.search_no_matches_body, ui.query),
                modifier = Modifier.fillMaxSize(),
            )
            else -> LazyColumn(Modifier.fillMaxHeight().responsiveContentWidth().padding(horizontal = 8.dp, vertical = 4.dp)) {
                items(ui.results.size) { i ->
                    val r = ui.results[i]
                    ResultRow(r, ui.query) { pick(r) }
                }
            }
        }
    }
}

@Composable
private fun RecentRow(rs: com.sigmundgranaas.turbo.expressive.domain.RecentSearch, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(TurboRadius.l)).clickable(onClick = onClick).padding(horizontal = 8.dp, vertical = 12.dp),
    ) {
        Cookie(size = 44.dp, fill = cs.surfaceContainerHigh) { Icon(Icons.Rounded.History, null, tint = cs.onSurfaceVariant, modifier = Modifier.size(22.dp)) }
        Spacer(Modifier.width(14.dp))
        Column(Modifier.weight(1f)) {
            Text(rs.name, style = MaterialTheme.typography.titleMedium, color = cs.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (rs.sub.isNotBlank()) {
                Text(rs.sub, style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        Icon(Icons.Rounded.NorthWest, null, tint = cs.onSurfaceVariant)
    }
}

@Composable
private fun ResultRow(r: SearchResult, query: String, onClick: () -> Unit) {
    val cs = MaterialTheme.colorScheme
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(TurboRadius.l)).clickable(onClick = onClick).padding(horizontal = 8.dp, vertical = 12.dp),
    ) {
        Cookie(size = 44.dp, fill = cs.surfaceContainerHigh) { Icon(r.kind.icon, null, tint = cs.primary, modifier = Modifier.size(22.dp)) }
        Spacer(Modifier.width(14.dp))
        Column(Modifier.weight(1f)) {
            Text(highlightPrefix(r.name, query), style = MaterialTheme.typography.titleMedium, color = cs.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(r.sub, style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        Icon(Icons.Rounded.NorthWest, null, tint = cs.onSurfaceVariant)
    }
}

