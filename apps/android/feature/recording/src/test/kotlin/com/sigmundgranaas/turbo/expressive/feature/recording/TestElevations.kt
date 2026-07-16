package com.sigmundgranaas.turbo.expressive.feature.recording

import com.sigmundgranaas.turbo.expressive.core.common.Outcome
import com.sigmundgranaas.turbo.expressive.core.data.ElevationRepository
import com.sigmundgranaas.turbo.expressive.core.sync.LinkRedemption
import com.sigmundgranaas.turbo.expressive.core.sync.ResourceSyncPageDto
import com.sigmundgranaas.turbo.expressive.core.sync.SharingRepository
import com.sigmundgranaas.turbo.expressive.domain.LatLng

/** Elevation repo that always fails — backfill becomes a no-op, keeping tests
 *  that don't care about elevation independent of the network primitive. */
internal object NoElevations : ElevationRepository {
    override suspend fun sample(points: List<LatLng>): Outcome<List<Double?>> =
        Outcome.Failure(UnsupportedOperationException("no elevations in this test"))
}

/** Elevation repo answering a fixed profile (cycled to the request length). */
internal class FixedElevations(private val values: List<Double?>) : ElevationRepository {
    override suspend fun sample(points: List<LatLng>): Outcome<List<Double?>> =
        Outcome.Success(List(points.size) { i -> values[i % values.size] })
}

/** Sharing repo for tests that never touch sharing. */
internal object NoopSharing : SharingRepository {
    override suspend fun friendCode() = Outcome.Failure(RuntimeException())
    override suspend fun createLink(resourceId: String, role: String) = Outcome.Success("https://x/link/t")
    override suspend fun redeemLink(token: String) = Outcome.Success(LinkRedemption("r", "path", "viewer"))
    override suspend fun sharedResources(since: String?) = Outcome.Success(ResourceSyncPageDto())
}
