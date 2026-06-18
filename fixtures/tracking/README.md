# Tracking fixtures — the cross-platform contract

Language-agnostic golden fixtures for the tracking/following redesign
(`docs/architecture/2026-06-tracking-following-redesign.md`). Both the iOS and
Android unit tests load these **exact files** and assert against them, so the two
native implementations can't silently diverge.

Geometry is kept deliberately simple (equator, mostly east-west legs) so the
expected values are analytically obvious and human-verifiable. At the equator a
pure-longitude move of `d` degrees is `≈ 111195 · d` metres (haversine, R =
6 371 000 m).

## `progress/*.json` — RouteProgress cursor (US-2/US-3)

```jsonc
{
  "name": "...",
  "description": "...",
  "params": { "windowBackM": 60, "windowAheadM": 400, "offRouteM": 50, "arriveEndM": 30 },
  "route": [[lat, lng], ...],          // optional 3rd element = elevation (m)
  "fixes": [[lat, lng], ...],          // fed in order; the cursor is stateful
  "expect": [                          // one entry per fix, same order
    { "fraction": 0.0, "arrived": false, "offRoute": false },
    ...
  ]
}
```

`fraction` is asserted with tolerance `±0.02`. `arrived`/`offRoute` are exact.
The point of `out-and-back` is that a return-leg fix sharing coordinates with an
outbound fix must report the *later* fraction (the global-nearest algorithm gets
this wrong; the arc-length cursor gets it right).

## `filter/*.json` — LocationFilter (US-5)

```jsonc
{
  "name": "...",
  "description": "...",
  "params": { "accuracyMaxM": 50, "stalenessMaxMs": 5000, "jumpMaxMps": 30 },
  "fixes": [
    { "lat": .., "lng": .., "accuracyM": .., "ageMs": .., "speedMps": .. },
    ...
  ],
  "acceptedIndices": [0, 2, 3]        // indices of fixes the filter should accept
}
```
