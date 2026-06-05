package com.sigmundgranaas.turbo.expressive.core.common

/**
 * A lightweight success/failure result for repository + use-case boundaries,
 * so the UI can render Loading/Content/Error without exceptions leaking up.
 */
sealed interface Outcome<out T> {
    data class Success<T>(val value: T) : Outcome<T>
    data class Failure(val error: Throwable) : Outcome<Nothing>

    fun getOrNull(): T? = (this as? Success)?.value

    fun <R> map(transform: (T) -> R): Outcome<R> = when (this) {
        is Success -> Success(transform(value))
        is Failure -> this
    }

    companion object {
        inline fun <T> catching(block: () -> T): Outcome<T> =
            try {
                Success(block())
            } catch (t: Throwable) {
                Failure(t)
            }
    }
}
