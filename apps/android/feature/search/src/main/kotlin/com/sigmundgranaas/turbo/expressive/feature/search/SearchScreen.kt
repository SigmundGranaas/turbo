package com.sigmundgranaas.turbo.expressive.feature.search

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.NorthWest
import androidx.compose.material.icons.rounded.Place
import androidx.compose.material.icons.rounded.Public
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.sigmundgranaas.turbo.expressive.ui.components.Cookie
import com.sigmundgranaas.turbo.expressive.ui.theme.TurboRadius
import com.sigmundgranaas.turbo.expressive.ui.theme.icon

/**
 * Bold only the leading [query] when [name] actually starts with it (case-insensitive);
 * otherwise render the name plain. Avoids the "StorSjurfjellet" artifact where a
 * non-matching name had the query blindly prepended in bold.
 */
private fun highlightPrefix(name: String, query: String): AnnotatedString = buildAnnotatedString {
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
    viewModel: SearchViewModel = hiltViewModel(),
) {
    val cs = MaterialTheme.colorScheme
    val ui by viewModel.state.collectAsStateWithLifecycle()

    Column(Modifier.fillMaxSize().background(cs.surface)) {
        Surface(
            shape = CircleShape, color = cs.surfaceContainerHigh, shadowElevation = 1.dp,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 14.dp).fillMaxWidth().height(56.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(horizontal = 8.dp)) {
                IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, "Back", tint = cs.onSurface) }
                Text(ui.query, style = MaterialTheme.typography.bodyLarge, color = cs.onSurface, modifier = Modifier.weight(1f).padding(horizontal = 4.dp))
                IconButton(onClick = onBack) { Icon(Icons.Rounded.Close, "Clear", tint = cs.onSurfaceVariant) }
            }
        }

        Row(Modifier.padding(horizontal = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FilterChip(selected = ui.filter == 0, onClick = { viewModel.setFilter(0) }, label = { Text("All") })
            FilterChip(selected = ui.filter == 1, onClick = { viewModel.setFilter(1) }, label = { Text("Markers") }, leadingIcon = { Icon(Icons.Rounded.Place, null, Modifier.size(18.dp)) })
            FilterChip(selected = ui.filter == 2, onClick = { viewModel.setFilter(2) }, label = { Text("Places") }, leadingIcon = { Icon(Icons.Rounded.Public, null, Modifier.size(18.dp)) })
        }

        Column(Modifier.padding(horizontal = 8.dp, vertical = 4.dp)) {
            ui.results.forEach { r ->
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(TurboRadius.l)).padding(horizontal = 8.dp, vertical = 12.dp)) {
                    Cookie(size = 44.dp, fill = cs.surfaceContainerHigh) { Icon(r.kind.icon, null, tint = cs.primary, modifier = Modifier.size(22.dp)) }
                    Spacer(Modifier.width(14.dp))
                    Column(Modifier.weight(1f)) {
                        Text(
                            highlightPrefix(r.name, ui.query),
                            style = MaterialTheme.typography.titleMedium,
                            color = cs.onSurface,
                        )
                        Text(r.sub, style = MaterialTheme.typography.bodySmall, color = cs.onSurfaceVariant)
                    }
                    Icon(Icons.Rounded.NorthWest, null, tint = cs.onSurfaceVariant)
                }
            }
        }
    }
}
