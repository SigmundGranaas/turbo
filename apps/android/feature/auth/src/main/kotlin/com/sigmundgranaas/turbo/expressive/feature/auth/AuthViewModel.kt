package com.sigmundgranaas.turbo.expressive.feature.auth

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.sigmundgranaas.turbo.expressive.core.auth.AuthRepository
import com.sigmundgranaas.turbo.expressive.core.auth.AuthState
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

enum class AuthMode { Login, Register }

/** Editable state of the sign-in form. */
data class AuthFormState(
    val mode: AuthMode = AuthMode.Login,
    val email: String = "",
    val password: String = "",
    val confirm: String = "",
    val loading: Boolean = false,
    val error: String? = null,
) {
    val canSubmit: Boolean
        get() = !loading && email.isNotBlank() && password.length >= MIN_PASSWORD &&
            (mode == AuthMode.Login || password == confirm)

    private companion object { const val MIN_PASSWORD = 6 }
}

@HiltViewModel
class AuthViewModel @Inject constructor(
    private val repo: AuthRepository,
) : ViewModel() {

    val state: StateFlow<AuthState> = repo.state

    var form by mutableStateOf(AuthFormState())
        private set

    init {
        viewModelScope.launch {
            // Fast local restore first (guest-friendly, no network on the way to the
            // map), then validate the session against the server in the background —
            // a revocation/disabled account flips to SignedOut instead of leaving the
            // app "signed in" with silently failing sync.
            repo.restore()
            repo.validateSession()
        }
    }

    fun onEmail(v: String) { form = form.copy(email = v, error = null) }
    fun onPassword(v: String) { form = form.copy(password = v, error = null) }
    fun onConfirm(v: String) { form = form.copy(confirm = v, error = null) }

    fun toggleMode() {
        form = form.copy(
            mode = if (form.mode == AuthMode.Login) AuthMode.Register else AuthMode.Login,
            confirm = "",
            error = null,
        )
    }

    fun submit() {
        if (!form.canSubmit) return
        val current = form
        form = current.copy(loading = true, error = null)
        viewModelScope.launch {
            val result = when (current.mode) {
                AuthMode.Login -> repo.login(current.email.trim(), current.password)
                AuthMode.Register -> repo.register(current.email.trim(), current.password)
            }
            // On success the AuthState flips to SignedIn and the screen navigates away;
            // only surface the error path here.
            if (result is Outcome.Failure) {
                form = form.copy(loading = false, error = result.error.message)
            }
        }
    }

    /** Fetch the Google consent URL, then hand it to [open] (a Custom Tab / browser). */
    fun beginGoogleSignIn(open: (String) -> Unit) {
        form = form.copy(loading = true, error = null)
        viewModelScope.launch {
            when (val url = repo.googleAuthUrl()) {
                is Outcome.Success -> { form = form.copy(loading = false); open(url.value) }
                is Outcome.Failure -> form = form.copy(loading = false, error = url.error.message)
            }
        }
    }

    // NOTE: the OAuth redirect (turbo://oauth?code=…) is finished by MainActivity's
    // deep-link handler calling AuthRepository.loginWithGoogle directly — this
    // ViewModel only STARTS the flow (beginGoogleSignIn); it never sees the code.

    fun logout() {
        viewModelScope.launch { repo.logout() }
    }
}
