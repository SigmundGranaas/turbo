package com.sigmundgranaas.turbo.expressive.ui.components

import android.os.Build
import android.view.HapticFeedbackConstants
import android.view.View
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalView

/**
 * One semantic haptics vocabulary for the whole app, so every screen buzzes the
 * same way for the same kind of moment instead of each feature inventing (or
 * skipping) its own feedback.
 *
 * Built on [View.performHapticFeedback] with the richer constants gated by API
 * level and a graceful fall back to [HapticFeedbackConstants.LONG_PRESS] on older
 * devices. Obtain one with [rememberTurboHaptics].
 */
class TurboHaptics(private val view: View) {

    /** Picking something up / a long-press creation gesture (drop a marker). */
    fun longPress() = view.perform(HapticFeedbackConstants.LONG_PRESS)

    /** Flipping a switch or toggle (map overlays, settings). */
    fun toggle(on: Boolean) = view.perform(
        when {
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE ->
                if (on) HapticFeedbackConstants.TOGGLE_ON else HapticFeedbackConstants.TOGGLE_OFF
            else -> HapticFeedbackConstants.CLOCK_TICK
        },
    )

    /** A positive, committing action succeeded (start recording, save). */
    fun confirm() = view.perform(
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) HapticFeedbackConstants.CONFIRM
        else HapticFeedbackConstants.LONG_PRESS,
    )

    /** A destructive / negative commit (delete, stop). A firmer bump. */
    fun reject() = view.perform(
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) HapticFeedbackConstants.REJECT
        else HapticFeedbackConstants.LONG_PRESS,
    )

    private fun View.perform(constant: Int) {
        performHapticFeedback(constant)
    }
}

@Composable
fun rememberTurboHaptics(): TurboHaptics {
    val view = LocalView.current
    return remember(view) { TurboHaptics(view) }
}
