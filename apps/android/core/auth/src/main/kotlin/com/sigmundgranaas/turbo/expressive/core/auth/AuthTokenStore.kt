package com.sigmundgranaas.turbo.expressive.core.auth

import android.content.Context
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.first
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Persists the auth tokens + account across launches. An interface so the
 * repository can be unit-tested with an in-memory fake (DataStore needs a Context).
 *
 * NOTE: backed by plain DataStore for now; hardening the refresh token to
 * EncryptedSharedPreferences / Keystore is a deliberate later (P5) step.
 */
interface AuthTokenStore {
    suspend fun save(tokens: AuthTokens, account: Account)
    suspend fun tokens(): AuthTokens?
    suspend fun account(): Account?
    suspend fun clear()
}

private val Context.authDataStore by preferencesDataStore(name = "turbo_auth")

@Singleton
class DataStoreAuthTokenStore @Inject constructor(
    @param:ApplicationContext private val context: Context,
) : AuthTokenStore {
    private val store = context.authDataStore

    override suspend fun save(tokens: AuthTokens, account: Account) {
        store.edit {
            it[ACCESS] = tokens.accessToken
            it[REFRESH] = tokens.refreshToken
            it[ACCOUNT_ID] = account.id
            it[EMAIL] = account.email
        }
    }

    override suspend fun tokens(): AuthTokens? {
        val prefs = store.data.first()
        val access = prefs[ACCESS] ?: return null
        val refresh = prefs[REFRESH] ?: return null
        return AuthTokens(access, refresh)
    }

    override suspend fun account(): Account? {
        val prefs = store.data.first()
        val id = prefs[ACCOUNT_ID] ?: return null
        val email = prefs[EMAIL] ?: return null
        return Account(id, email)
    }

    override suspend fun clear() {
        store.edit { it.clear() }
    }

    private companion object {
        val ACCESS = stringPreferencesKey("access_token")
        val REFRESH = stringPreferencesKey("refresh_token")
        val ACCOUNT_ID = stringPreferencesKey("account_id")
        val EMAIL = stringPreferencesKey("email")
    }
}
