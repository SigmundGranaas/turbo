# Activities — region polygon seeds

GeoJSON files in this folder are loaded into `activities.geo_regions` by
`GeoRegionSeederHostedService` at host startup. Each file is a standard
`FeatureCollection`; each feature becomes one row keyed on
`(source, region_id)`.

Configure the seeder via `appsettings.json`:

```json
{
  "GeoRegionSeeder": {
    "Sources": [
      {
        "Source": "varsom_region",
        "FilePath": "/app/seed/varsom-regions.geojson",
        "RegionIdProperty": "OmradeId",
        "NameProperty": "OmradeNavn"
      },
      {
        "Source": "watershed",
        "FilePath": "/app/seed/regine-watersheds.geojson",
        "RegionIdProperty": "vassdragsNr",
        "NameProperty": "vassdragsNavn"
      },
      {
        "Source": "mareano_cell",
        "FilePath": "/app/seed/mareano-cells.geojson",
        "RegionIdProperty": "cellId",
        "NameProperty": "cellName"
      }
    ]
  }
}
```

When the seed file is missing the service logs a warning and skips that
source — the orchestrators degrade gracefully when no polygons are
loaded (Varsom-region-derived drivers fall back to "verify Varsom
before going").

## What's committed

### `varsom-regions.geojson` — Norway's 24 avalanche warning regions

Source: https://api01.nve.no/hydrology/forecast/avalanche/v6.3.0/api/Region
(TypeName="A" entries). The script that produced this file is in
`scripts/build-varsom-regions.py`. Re-run when the region taxonomy
changes (rare — last redrawn 2013).

`RegionIdProperty: OmradeId` (3001–3037 — the same ids the Varsom
warning API uses, so `BackcountrySkiOrchestrator` can look up the
right bulletin once `ActivityGeoContextService.VarsomRegionId` is
populated).

## What's NOT committed (too large)

### REGINE watersheds (`regine-watersheds.geojson`)

NVE's REGINE catchment database has ~20,000 polygons (tens of MB
uncompressed). Download from Geonorge:

```
https://kartkatalog.geonorge.no/metadata/regine---vassdragsenhet/8721cdac-f959-4adc-9d54-d3b770e5fa1e
```

Pick "GeoJSON" + "Hele landet" (all of Norway) in the download form,
or use the WFS endpoint:

```
https://wfs.geonorge.no/skwms1/wfs.regine?service=WFS&version=2.0.0
  &request=GetFeature&typeNames=app:Vassdragsomrade
  &outputFormat=application/json
```

Save as `seed/regine-watersheds.geojson`, configure the seeder source
with `RegionIdProperty: "vassdragsNr"`. Used by orchestrators for
cross-activity signal sharing on shared watersheds (freediving viz +
fishing post-spate + packrafting flow context all reference the same
upstream catchment).

### Mareano seabed cells (`mareano-cells.geojson`)

Norwegian Mapping Authority's Mareano dataset. Substrate + bathymetry
gridded into named cells covering the Norwegian coast and continental
shelf. Several hundred MB at full resolution; for our purposes the
"coastal substrate overview" is enough. Download from:

```
https://kartkatalog.geonorge.no/metadata/uuid/dee9aa15-3733-4d3b-9b32-87ad6918a3d8
```

Used by `FreedivingOrchestrator` to derive bottom-type and depth-band
features from geometry (replaces the user-entered `bottomType` field).

## Re-running the seeder

The hosted service runs once per host start. To force a re-import,
restart the host. The seeder is idempotent — re-runs UPDATE rows for
keys that already exist, INSERT new ones, leave nothing else alone.
