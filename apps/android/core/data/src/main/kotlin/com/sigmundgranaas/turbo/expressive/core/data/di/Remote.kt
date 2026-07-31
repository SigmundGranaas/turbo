package com.sigmundgranaas.turbo.expressive.core.data.di

import javax.inject.Qualifier

/**
 * The **network** router, before any on-device fallback wraps it.
 *
 * Two decisions were being made by one binding, and they belong to
 * different modules. Which *remote* router to use — the real SSE
 * pathfinder, or the synthetic stand-in on an emulator where it is
 * unreachable — is a build-variant question and stays in `:core:data`.
 * Whether to fall back to the phone is a capability question and lives
 * with the code that can answer it.
 *
 * Without the qualifier those two bindings compete for the same type and
 * the app fails to build with a duplicate binding — which is the
 * annotation processor doing its job, and how this was found.
 */
@Qualifier
@Retention(AnnotationRetention.BINARY)
annotation class Remote
