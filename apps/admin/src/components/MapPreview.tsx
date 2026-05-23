import { useEffect, useRef } from "react";
import maplibregl from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";

import type { Resource } from "../api/types";

const RESOURCE_COLOR: Record<Resource, string> = {
  "hiking-trails": "#E53935",
  "ski-tracks": "#1E88E5",
  "forest-roads": "#6D4C41",
  "cycling-routes": "#43A047",
};

/**
 * MapLibre GL preview of one curated route's geometry. Renders the
 * MVT tiles served by this resource as a backdrop and overlays the
 * route's geometry on top so curators see how the route fits into the
 * underlying path network.
 */
export function MapPreview({
  geometry,
  resource,
}: {
  geometry: GeoJSON.Geometry;
  resource: string;
}) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const mapRef = useRef<maplibregl.Map | null>(null);

  useEffect(() => {
    if (!containerRef.current || mapRef.current) return;
    const map = new maplibregl.Map({
      container: containerRef.current,
      style: {
        version: 8,
        sources: {
          osm: {
            type: "raster",
            tiles: ["https://tile.openstreetmap.org/{z}/{x}/{y}.png"],
            tileSize: 256,
            attribution: "© OpenStreetMap",
          },
        },
        layers: [{ id: "osm", type: "raster", source: "osm" }],
      },
      center: [10.75, 59.91],
      zoom: 8,
    });
    mapRef.current = map;
    return () => {
      map.remove();
      mapRef.current = null;
    };
  }, []);

  useEffect(() => {
    const map = mapRef.current;
    if (!map || !geometry) return;
    const draw = () => {
      const color = RESOURCE_COLOR[resource as Resource] ?? "#1c1917";
      if (map.getSource("route")) {
        (map.getSource("route") as maplibregl.GeoJSONSource).setData({
          type: "Feature",
          geometry,
          properties: {},
        });
      } else {
        map.addSource("route", {
          type: "geojson",
          data: { type: "Feature", geometry, properties: {} },
        });
        map.addLayer({
          id: "route-line",
          type: "line",
          source: "route",
          paint: {
            "line-color": color,
            "line-width": 4,
            "line-opacity": 0.9,
          },
        });
      }
      const bounds = boundingBox(geometry);
      if (bounds) {
        map.fitBounds(bounds as [[number, number], [number, number]], {
          padding: 24,
          maxZoom: 14,
        });
      }
    };
    if (map.loaded()) draw();
    else map.on("load", draw);
  }, [geometry, resource]);

  return (
    <div
      ref={containerRef}
      className="w-full h-[480px] rounded border border-ink-200 overflow-hidden"
    />
  );
}

function boundingBox(geom: GeoJSON.Geometry): number[][] | null {
  let minLon = Infinity,
    minLat = Infinity,
    maxLon = -Infinity,
    maxLat = -Infinity;
  const walk = (coords: GeoJSON.Position[]) => {
    for (const c of coords) {
      const [lon, lat] = c;
      if (lon < minLon) minLon = lon;
      if (lat < minLat) minLat = lat;
      if (lon > maxLon) maxLon = lon;
      if (lat > maxLat) maxLat = lat;
    }
  };
  if (geom.type === "LineString") walk(geom.coordinates);
  else if (geom.type === "MultiLineString")
    geom.coordinates.forEach(walk);
  else if (geom.type === "Polygon") geom.coordinates.forEach(walk);
  else return null;
  if (!isFinite(minLon)) return null;
  return [
    [minLon, minLat],
    [maxLon, maxLat],
  ];
}
