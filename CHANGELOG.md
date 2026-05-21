# Changelog

All notable changes to Turkart will be documented in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.23]

### Added

- Activities feature: six kinds (fishing, backcountry skiing, hiking, XC
  skiing, packrafting, freediving) each with their own typed form,
  detail view, and conditions panel composed from the shared shell.
- Offline-first activities: cross-kind summaries and per-kind detail +
  conditions reports persist in SQLite, hydrating the map on cold start
  and falling back to the last-known payload when the network drops.
  Connectivity reconnect triggers an automatic delta refresh.
- Activity create picker dispatches to each kind's create screen via a
  registry, renders a sign-in CTA for anonymous users, and is reachable
  from the long-press menu, marker "save as activity", and saved-path
  promotion.
- Route-drawing surface for line-shaped kinds with tap-to-add,
  drag-to-move, and long-press-to-remove vertices.
- Behaviour + widget tests covering the offline pipeline (summary
  hydration, cache fallback, connectivity reconnect, tombstone cache
  eviction) and the picker's auth-state branching.

## [1.0.22]

### Changed

- Routed core (`api_client`, `app`, `location_state`) logging through the
  shared `package:logging` Logger so output is uniformly gated to debug
  builds via `setupLogging()`.
- Aligned the web PWA manifest with the actual app (real name,
  description, and theme color matching the in-app primary).

### Fixed

- Async error states in markers, paths, settings, collections, trip
  stats, and marker photos no longer show the raw exception string;
  they show a localized friendly message in both English and Norwegian.
- Localized the offline-regions "Cleanup" tooltip and the Google OAuth
  callback "Unknown error" fallback that were still in English.

## [1.0.21]

Baseline at which this changelog starts. See git history for the full
sequence of changes — major prior milestones include live GPS recording
with elevation profiles, collections, the design-system unification,
weather data integration, and the move to a strict feature-oriented
architecture with enforced boundary tests.
