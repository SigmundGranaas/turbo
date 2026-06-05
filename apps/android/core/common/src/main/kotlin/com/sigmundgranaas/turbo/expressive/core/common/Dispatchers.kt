package com.sigmundgranaas.turbo.expressive.core.common

import javax.inject.Qualifier

/** Qualifies an injected [kotlinx.coroutines.CoroutineDispatcher]. */
@Qualifier
@Retention(AnnotationRetention.RUNTIME)
annotation class Dispatcher(val dispatcher: TurboDispatcher)

enum class TurboDispatcher { Default, IO }
