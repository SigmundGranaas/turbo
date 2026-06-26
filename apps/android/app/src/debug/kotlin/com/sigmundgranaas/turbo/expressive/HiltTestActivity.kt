package com.sigmundgranaas.turbo.expressive

import androidx.activity.ComponentActivity
import dagger.hilt.android.AndroidEntryPoint

/**
 * An empty, Hilt-enabled host activity for headless Compose E2E tests. Tests
 * `setContent { … }` the real app root on it, so `hiltViewModel()` resolves
 * against a genuine `@AndroidEntryPoint` activity. Debug-only — never shipped.
 */
@AndroidEntryPoint
class HiltTestActivity : ComponentActivity()
