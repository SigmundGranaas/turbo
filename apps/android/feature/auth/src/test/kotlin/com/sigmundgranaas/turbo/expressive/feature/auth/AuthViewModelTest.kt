package com.sigmundgranaas.turbo.expressive.feature.auth

import com.sigmundgranaas.turbo.expressive.core.auth.Account
import com.sigmundgranaas.turbo.expressive.core.auth.AuthException
import com.sigmundgranaas.turbo.expressive.core.auth.AuthRepository
import com.sigmundgranaas.turbo.expressive.core.auth.AuthState
import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class AuthViewModelTest {

    private val dispatcher = StandardTestDispatcher()

    private class FakeAuthRepository(var result: Outcome<Account>) : AuthRepository {
        override val state = MutableStateFlow<AuthState>(AuthState.Unknown)
        var loginCalls = 0
        var registerCalls = 0
        override suspend fun restore() { state.value = AuthState.SignedOut }
        override suspend fun register(email: String, password: String): Outcome<Account> {
            registerCalls++; return apply(result)
        }
        override suspend fun login(email: String, password: String): Outcome<Account> {
            loginCalls++; return apply(result)
        }
        override suspend fun googleAuthUrl() = Outcome.Success("https://x")
        override suspend fun loginWithGoogle(code: String) = apply(result)
        override suspend fun refresh() = Outcome.Success(Unit)
        override suspend fun logout() { state.value = AuthState.SignedOut }
        override suspend fun accessToken(): String? = null
        private fun apply(r: Outcome<Account>): Outcome<Account> {
            (r as? Outcome.Success)?.let { state.value = AuthState.SignedIn(it.value) }
            return r
        }
    }

    @Before fun setUp() = Dispatchers.setMain(dispatcher)
    @After fun tearDown() = Dispatchers.resetMain()

    @Test
    fun `login submits and flips state to signed-in`() = runTest(dispatcher) {
        val repo = FakeAuthRepository(Outcome.Success(Account("a1", "h@b.no")))
        val vm = AuthViewModel(repo)
        vm.onEmail("h@b.no"); vm.onPassword("secret1")
        assertTrue(vm.form.canSubmit)
        vm.submit()
        advanceUntilIdle()
        assertEquals(1, repo.loginCalls)
        assertTrue(vm.state.value is AuthState.SignedIn)
    }

    @Test
    fun `a failed sign-in surfaces the error and clears loading`() = runTest(dispatcher) {
        val repo = FakeAuthRepository(Outcome.Failure(AuthException("Invalid credentials")))
        val vm = AuthViewModel(repo)
        vm.onEmail("h@b.no"); vm.onPassword("secret1")
        vm.submit()
        advanceUntilIdle()
        assertFalse(vm.form.loading)
        assertEquals("Invalid credentials", vm.form.error)
    }

    @Test
    fun `register requires a matching confirmation`() = runTest(dispatcher) {
        val vm = AuthViewModel(FakeAuthRepository(Outcome.Success(Account("a", "e"))))
        vm.toggleMode() // → Register
        vm.onEmail("h@b.no"); vm.onPassword("secret1"); vm.onConfirm("different")
        assertFalse(vm.form.canSubmit)
        vm.onConfirm("secret1")
        assertTrue(vm.form.canSubmit)
    }

    @Test
    fun `short passwords cannot submit`() = runTest(dispatcher) {
        val vm = AuthViewModel(FakeAuthRepository(Outcome.Success(Account("a", "e"))))
        vm.onEmail("h@b.no"); vm.onPassword("123")
        assertFalse(vm.form.canSubmit)
    }
}
