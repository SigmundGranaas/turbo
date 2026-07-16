package com.sigmundgranaas.turbo.expressive.feature.settings

import com.sigmundgranaas.turbo.expressive.core.auth.Account
import com.sigmundgranaas.turbo.expressive.core.auth.AuthRepository
import com.sigmundgranaas.turbo.expressive.core.auth.AuthState
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

/** Auth repo for settings tests: an observable state, everything else inert. */
internal class FakeAuthRepository(initial: AuthState = AuthState.SignedOut) : AuthRepository {
    val stateFlow = MutableStateFlow(initial)
    override val state: StateFlow<AuthState> = stateFlow
    override suspend fun restore() = Unit
    override suspend fun register(email: String, password: String): Outcome<Account> = fail()
    override suspend fun login(email: String, password: String): Outcome<Account> = fail()
    override suspend fun googleAuthUrl(): Outcome<String> = fail()
    override suspend fun loginWithGoogle(code: String): Outcome<Account> = fail()
    override suspend fun refresh(): Outcome<Unit> = fail()
    override suspend fun logout() = Unit
    override suspend fun accessToken(): String? = null

    private fun <T> fail(): Outcome<T> = Outcome.Failure(UnsupportedOperationException("not in this test"))
}
