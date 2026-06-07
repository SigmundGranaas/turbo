package com.sigmundgranaas.turbo.expressive.core.auth.di

import com.sigmundgranaas.turbo.expressive.core.auth.AuthClient
import com.sigmundgranaas.turbo.expressive.core.auth.AuthRepository
import com.sigmundgranaas.turbo.expressive.core.auth.AuthTokenStore
import com.sigmundgranaas.turbo.expressive.core.auth.DataStoreAuthTokenStore
import com.sigmundgranaas.turbo.expressive.core.auth.KtorAuthRepository
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import io.ktor.client.HttpClient
import io.ktor.client.engine.okhttp.OkHttp
import io.ktor.client.plugins.contentnegotiation.ContentNegotiation
import io.ktor.serialization.kotlinx.json.json
import kotlinx.serialization.json.Json
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
object AuthModule {
    /**
     * Dedicated client for the Turbo auth/app API — qualified so it never clashes
     * with the unqualified public map client in :core:data.
     */
    @Provides
    @Singleton
    @AuthClient
    fun provideAuthHttpClient(): HttpClient = HttpClient(OkHttp) {
        expectSuccess = false
        install(ContentNegotiation) {
            json(Json { ignoreUnknownKeys = true })
        }
    }

    @Provides
    @Singleton
    fun provideAuthTokenStore(impl: DataStoreAuthTokenStore): AuthTokenStore = impl

    @Provides
    @Singleton
    fun provideAuthRepository(impl: KtorAuthRepository): AuthRepository = impl
}
