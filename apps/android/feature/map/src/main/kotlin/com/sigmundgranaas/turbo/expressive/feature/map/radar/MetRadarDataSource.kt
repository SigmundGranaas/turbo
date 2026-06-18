package com.sigmundgranaas.turbo.expressive.feature.map.radar

import com.sigmundgranaas.turbo.expressive.domain.GeoBounds

/**
 * Live MET Norway radar/nowcast source — the production path, same provider as
 * the app's existing weather (`api.met.no`).
 *
 * **Status: skeleton.** The fetch is straightforward; the georeferencing is the
 * real work, and it needs an on-device GPU to verify, so it's deliberately left
 * unimplemented rather than shipped half-working. The wired default is
 * [SyntheticRadarDataSource]; swap this in once the steps below are done and
 * checked on a device.
 *
 * Implementation outline:
 * 1. **Precipitation** — `GET https://api.met.no/weatherapi/radar/2.0/?area={area}
 *    &type=reflectivity&content=image` returns a PNG for a *fixed* national area
 *    in EPSG:3575 (polar stereographic), with a known bounding box per area.
 *    Pick the area covering [bounds]. The animation endpoint returns the recent
 *    frame sequence (≈5-min steps) — one [RadarFrameData] each.
 * 2. **Coverage** — `cloud_area_fraction` from the AROME grid (or the
 *    locationforecast grid) sampled over the same cells; or derive a coarse
 *    coverage from where precip > 0 plus a forecast cloud field.
 * 3. **Georeference + resample** — for each of the `gridW * gridH` cells, take
 *    its lat/lon within [bounds] (Web-Mercator), project into the radar image's
 *    CRS, bilinear-sample the decoded image, and map the reflectivity/dBZ to
 *    `0..255`. Honour the User-Agent policy (`kTurboUserAgent`) and the
 *    `Expires`/`If-Modified-Since` cache headers, like `YrAtmosphericService`.
 *
 * Use a shared `OkHttpClient` and run decoding off the main thread.
 */
class MetRadarDataSource : RadarDataSource {
    override suspend fun load(bounds: GeoBounds, frameCount: Int): List<RadarFrameData> {
        throw NotImplementedError(
            "MetRadarDataSource is a documented skeleton — see the class KDoc for the " +
                "fetch + georeference steps. Use SyntheticRadarDataSource until it's wired.",
        )
    }
}
