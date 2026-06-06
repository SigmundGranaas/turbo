package com.sigmundgranaas.turbo.expressive.core.data

import android.content.Context
import com.sigmundgranaas.turbo.expressive.core.common.StringProvider
import dagger.hilt.android.qualifiers.ApplicationContext
import javax.inject.Inject

/** [StringProvider] backed by the application [Context]'s resources. */
class AndroidStringProvider @Inject constructor(
    @param:ApplicationContext private val context: Context,
) : StringProvider {
    override fun get(id: Int): String = context.getString(id)
    override fun get(id: Int, vararg formatArgs: Any): String = context.getString(id, *formatArgs)
}
