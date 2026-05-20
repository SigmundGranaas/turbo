# Changelog

All notable changes to Turkart will be documented in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
