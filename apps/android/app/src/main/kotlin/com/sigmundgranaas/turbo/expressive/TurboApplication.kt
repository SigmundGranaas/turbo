package com.sigmundgranaas.turbo.expressive

import android.app.Application
import com.sigmundgranaas.turbo.expressive.core.sync.SyncEngine
import dagger.hilt.android.HiltAndroidApp
import javax.inject.Inject

@HiltAndroidApp
class TurboApplication : Application() {

    @Inject lateinit var syncEngine: SyncEngine

    override fun onCreate() {
        super.onCreate()
        // Begin reacting to auth: sync on sign-in, drop cursors on sign-out.
        syncEngine.start()
    }
}
