import 'package:flutter/material.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import '../models/vector_feature.dart';

/// Decoded snapshot of a Turrutebasen feature's properties — projected
/// from raw SOSI codes into UI-ready strings + typed bits.
///
/// The decoder lives next to the trail sheet because all of its
/// vocabulary (Norwegian code tables for `merking`, `gradering`,
/// `preparering`, `sesong`) is a presentation concern: the underlying
/// `VectorFeature.properties` keeps the raw codes intact for anything
/// else that wants them.
class TrailProperties {
  /// Real human-readable name, with placeholders ("Ukjent" / "Unknown")
  /// stripped out. `null` when no usable name was supplied.
  final String? title;

  /// Route number e.g. "F_20170606" or "T9". Prefixed with `#` by the
  /// header — kept raw here.
  final String? routeNumber;

  /// Decoded marking status ("Marked", "Summer-marked", ...).
  final String? marking;

  /// Decoded difficulty with its colour cue.
  final TrailDifficulty? difficulty;

  /// Decoded season ("Summer", "Winter", "Year-round").
  final String? season;

  /// Decoded preparation method (ski tracks only — "Machine-groomed", ...).
  final String? preparation;

  /// Who maintains the route. Multiple maintainers in the same field are
  /// joined by `· `.
  final String? maintainer;

  /// Surface type label.
  final String? surface;

  /// Width in centimetres ("80 cm") when the source ships
  /// `<app:rutebredde>` as a number.
  final String? width;

  /// What the route physically follows ("Path", "Tractor road", ...).
  final String? follows;

  /// Free-form notes from `<app:informasjon>`.
  final String? notes;

  /// Data origin from `<app:opphav>` (e.g. "N50-kartdata").
  final String? source;

  /// Last-updated date, formatted as `yyyy-MM-dd`.
  final String? updated;

  const TrailProperties({
    this.title,
    this.routeNumber,
    this.marking,
    this.difficulty,
    this.season,
    this.preparation,
    this.maintainer,
    this.surface,
    this.width,
    this.follows,
    this.notes,
    this.source,
    this.updated,
  });

  bool get hasChips =>
      marking != null ||
      difficulty != null ||
      season != null ||
      preparation != null;

  bool get hasDetails =>
      maintainer != null ||
      surface != null ||
      width != null ||
      follows != null ||
      notes != null;

  /// Default factory — Turrutebasen SOSI vocabulary. Kept as
  /// `TrailProperties.from(...)` for source compatibility with the
  /// existing trail sheet builder; new sources should call the more
  /// explicit constructors below.
  factory TrailProperties.from(VectorFeature feature, BuildContext context) =>
      TrailProperties.fromTurrutebasen(feature, context);

  /// Project a Turrutebasen `app:Fotrute` / `app:Skiløype` / etc. feature
  /// through the SOSI code tables defined in the Geonorge schema.
  factory TrailProperties.fromTurrutebasen(
      VectorFeature feature, BuildContext context) {
    final l10n = context.l10n;
    final p = feature.properties;

    final rawName = _string(p['rutenavn']);
    final title =
        (rawName == null || _isPlaceholderName(rawName)) ? null : rawName;

    final maintainer = _splitMaintainer(_string(p['vedlikeholdsansvarlig']));

    final widthRaw = _string(p['rutebredde']);
    final width = _parseWidth(widthRaw);

    return TrailProperties(
      title: title,
      routeNumber: _string(p['rutenummer']),
      marking: _decodeMarking(_string(p['merking']), l10n),
      difficulty: _decodeDifficulty(_string(p['gradering']), l10n),
      season: _decodeSeason(_string(p['sesong']), l10n),
      preparation: _decodePreparation(_string(p['preparering']), l10n),
      maintainer: maintainer,
      surface: _string(p['underlagstype']),
      width: width,
      follows: _decodeRuteFolger(_string(p['ruteFølger']), l10n),
      notes: _string(p['informasjon']),
      source: _string(p['opphav']),
      updated: _formatDate(_string(p['oppdateringsdato'])),
    );
  }

  /// Project a Kartverket FKB Traktorveg+Sti feature
  /// (`ms:traktorveg_sti` / `ms:skogsbilveg`). This is the dataset that
  /// drives Norgeskart's basemap paths: terse SOSI-flavoured properties
  /// where most fields are blank. We surface what the live WFS actually
  /// ships — `typeveg` (sti / traktorveg) and the road system reference
  /// when present — and leave the rest of the sheet to the empty state.
  factory TrailProperties.fromN50Sti(
      VectorFeature feature, BuildContext context) {
    final p = feature.properties;
    final typeveg = _string(p['typeveg']);
    final vegnr = _string(p['vegsystemreferanse_vegnummer']);
    return TrailProperties(
      // FKB rows aren't curated routes; they're map segments. We never
      // synthesise a name — the sheet's "Unnamed route" fallback fires.
      title: null,
      routeNumber: vegnr,
      follows: _decodeFkbTypeveg(typeveg),
      source: 'Kartverket FKB',
    );
  }

  /// Project an OSM Overpass element. OSM uses key/value tags rather
  /// than the SOSI code tables; vocabulary mapping is documented in
  /// the [OpenStreetMap wiki](https://wiki.openstreetmap.org/wiki/Key:highway).
  factory TrailProperties.fromOsm(
      VectorFeature feature, BuildContext context) {
    final l10n = context.l10n;
    final p = feature.properties;
    return TrailProperties(
      title: _string(p['name']),
      routeNumber: _string(p['ref']),
      // OSM doesn't have a binary "marked" tag; presence of
      // `osmc:symbol` or `marked_trail=yes` is the closest proxy.
      marking: _decodeOsmMarking(p, l10n),
      difficulty: _decodeOsmDifficulty(_string(p['sac_scale']), l10n),
      season: _decodeOsmSeason(p, l10n),
      preparation: _decodeOsmPreparation(p, l10n),
      maintainer: _string(p['operator']),
      surface: _string(p['surface']),
      // `width` in OSM is metres; only show it for paths with an
      // explicit value, otherwise drop the row.
      width: _formatOsmWidth(_string(p['width'])),
      follows: _decodeOsmHighway(_string(p['highway']), context),
      notes: _string(p['description']) ?? _string(p['note']),
      // OSM is its own attribution; we don't surface a per-feature
      // source field here.
      source: 'OpenStreetMap',
    );
  }
}

/// Wraps a decoded `gradering` so the sheet can colour-tint the chip
/// in the same scale as DNT signage (green/blue/red/black).
class TrailDifficulty {
  final String label;
  final Color color;
  const TrailDifficulty({required this.label, required this.color});
}

// ─── decoders ─────────────────────────────────────────────────────────────

String? _decodeMarking(String? raw, AppLocalizations l10n) {
  if (raw == null) return null;
  switch (raw.toUpperCase()) {
    case 'JA':
      return l10n.trailMarkingYes;
    case 'NEI':
      return l10n.trailMarkingNo;
    case 'SM':
      return l10n.trailMarkingSummer;
    case 'VM':
      return l10n.trailMarkingWinter;
    case 'SVM':
      return l10n.trailMarkingAllSeason;
  }
  return raw;
}

TrailDifficulty? _decodeDifficulty(String? raw, AppLocalizations l10n) {
  if (raw == null) return null;
  switch (raw.toUpperCase()) {
    case 'G':
      return TrailDifficulty(
        label: l10n.trailDifficultyEasy,
        color: const Color(0xFF2E7D32),
      );
    case 'B':
      return TrailDifficulty(
        label: l10n.trailDifficultyModerate,
        color: const Color(0xFF1565C0),
      );
    case 'R':
      return TrailDifficulty(
        label: l10n.trailDifficultyDemanding,
        color: const Color(0xFFC62828),
      );
    case 'S':
      return TrailDifficulty(
        label: l10n.trailDifficultyExpert,
        color: const Color(0xFF212121),
      );
  }
  return TrailDifficulty(
    label: raw,
    color: const Color(0xFF424242),
  );
}

String? _decodeSeason(String? raw, AppLocalizations l10n) {
  if (raw == null) return null;
  switch (raw.toUpperCase()) {
    case 'S':
    case 'SH':
      return l10n.trailSeasonSummer;
    case 'V':
    case 'VH':
      return l10n.trailSeasonWinter;
    case 'H':
    case 'HA':
      return l10n.trailSeasonAllYear;
  }
  return raw;
}

String? _decodePreparation(String? raw, AppLocalizations l10n) {
  if (raw == null) return null;
  switch (raw.toUpperCase()) {
    case 'M':
      return l10n.trailPreparationGroomed;
    case 'U':
      return l10n.trailPreparationUngroomed;
    case 'S':
      return l10n.trailPreparationSnowmobile;
  }
  return raw;
}

String? _decodeRuteFolger(String? raw, AppLocalizations l10n) {
  if (raw == null) return null;
  // SOSI ruteFølger codes — the ones that actually appear in
  // Geonorge's Turrutebasen dataset.
  switch (raw.toUpperCase()) {
    case 'ST':
      return 'Sti';
    case 'TV':
      return 'Traktorvei';
    case 'V':
      return 'Vei';
    case 'SK':
      return 'Skogsbilvei';
    case 'TR':
      return 'Trapp';
    case 'BR':
      return 'Bro';
  }
  return raw;
}

// ─── helpers ──────────────────────────────────────────────────────────────

bool _isPlaceholderName(String s) {
  final lower = s.trim().toLowerCase();
  return lower.isEmpty || lower == 'ukjent' || lower == 'unknown';
}

String? _string(Object? v) {
  if (v == null) return null;
  final s = v.toString().trim();
  return s.isEmpty ? null : s;
}

/// `vedlikeholdsansvarlig` comes back as a pipe-delimited string when
/// more than one party maintains the route — e.g. "DNT | DNT Oslo og
/// omegn". The pipe is a SOSI artifact and unfriendly to read; reflow
/// to a middot-separated list.
String? _splitMaintainer(String? raw) {
  if (raw == null) return null;
  final parts = raw
      .split('|')
      .map((s) => s.trim())
      .where((s) => s.isNotEmpty)
      .toList();
  if (parts.isEmpty) return null;
  return parts.join(' · ');
}

String? _parseWidth(String? raw) {
  if (raw == null) return null;
  final n = num.tryParse(raw);
  if (n == null) return raw;
  return '${n.toStringAsFixed(0)} cm';
}

/// SOSI dates arrive as either `yyyy-MM-dd` or `yyyy-MM-ddTHH:mm:ss`.
/// We collapse to the date part to keep the footer terse.
String? _formatDate(String? raw) {
  if (raw == null) return null;
  final i = raw.indexOf('T');
  return (i > 0) ? raw.substring(0, i) : raw;
}

// ─── OSM tag decoders ─────────────────────────────────────────────────────

String? _decodeOsmMarking(Map<String, Object?> tags, AppLocalizations l10n) {
  // OSM convention: `osmc:symbol` carries the route-marker spec; its
  // mere presence implies a marked trail. `marked_trail=yes/no` is also
  // used regionally. `informal=yes` means an unofficial, unmarked path.
  if (tags['marked_trail'] == 'yes') return l10n.trailMarkingYes;
  if (tags['marked_trail'] == 'no' || tags['informal'] == 'yes') {
    return l10n.trailMarkingNo;
  }
  if (tags['osmc:symbol'] is String &&
      (tags['osmc:symbol'] as String).trim().isNotEmpty) {
    return l10n.trailMarkingYes;
  }
  return null;
}

TrailDifficulty? _decodeOsmDifficulty(
    String? sacScale, AppLocalizations l10n) {
  if (sacScale == null) return null;
  // SAC hiking scale — the canonical alpine difficulty grading used in
  // the Alps and adopted across European hiking communities. Mapping
  // chosen to match the DNT difficulty colours.
  switch (sacScale) {
    case 'hiking':
      return TrailDifficulty(
        label: l10n.trailDifficultyEasy,
        color: const Color(0xFF2E7D32),
      );
    case 'mountain_hiking':
      return TrailDifficulty(
        label: l10n.trailDifficultyModerate,
        color: const Color(0xFF1565C0),
      );
    case 'demanding_mountain_hiking':
      return TrailDifficulty(
        label: l10n.trailDifficultyDemanding,
        color: const Color(0xFFC62828),
      );
    case 'alpine_hiking':
    case 'demanding_alpine_hiking':
    case 'difficult_alpine_hiking':
      return TrailDifficulty(
        label: l10n.trailDifficultyExpert,
        color: const Color(0xFF212121),
      );
  }
  return null;
}

String? _decodeOsmSeason(Map<String, Object?> tags, AppLocalizations l10n) {
  // Piste tags imply winter usage.
  if (tags['piste:type'] is String) return l10n.trailSeasonWinter;
  if (tags['winter_road'] == 'yes' || tags['winter_only'] == 'yes') {
    return l10n.trailSeasonWinter;
  }
  if (tags['seasonal'] == 'summer') return l10n.trailSeasonSummer;
  return null;
}

String? _decodeOsmPreparation(
    Map<String, Object?> tags, AppLocalizations l10n) {
  final groomingValue = tags['piste:grooming'];
  if (groomingValue is! String) return null;
  switch (groomingValue) {
    case 'classic':
    case 'classic+skating':
    case 'skating':
    case 'mogul':
      return l10n.trailPreparationGroomed;
    case 'backcountry':
      return l10n.trailPreparationUngroomed;
    case 'scooter':
      return l10n.trailPreparationSnowmobile;
  }
  return null;
}

String? _decodeOsmHighway(String? highway, BuildContext context) {
  if (highway == null) return null;
  switch (highway) {
    case 'path':
      return 'Sti';
    case 'footway':
      return 'Gangsti';
    case 'track':
      return 'Traktorvei';
    case 'bridleway':
      return 'Ridesti';
    case 'cycleway':
      return 'Sykkelvei';
  }
  return highway;
}

String? _formatOsmWidth(String? raw) {
  if (raw == null) return null;
  final n = num.tryParse(raw);
  if (n == null) return raw; // pass through "1 m" / "2-3 m" / etc.
  // OSM `width` is metres; convert to "x m" for terseness.
  return '${n.toStringAsFixed(n == n.toInt() ? 0 : 1)} m';
}

// ─── FKB Traktorveg+Sti decoders ──────────────────────────────────────────

/// FKB `typeveg` codes observed on the live wms.traktorveg_skogsbilveger
/// service: "sti" (path), "traktorveg" (tractor road), "skogsbilveg"
/// (forest road).
String? _decodeFkbTypeveg(String? typeveg) {
  if (typeveg == null) return null;
  switch (typeveg.toLowerCase()) {
    case 'sti':
      return 'Sti';
    case 'traktorveg':
    case 'traktorvei':
      return 'Traktorvei';
    case 'skogsbilveg':
    case 'skogsbilvei':
      return 'Skogsbilvei';
  }
  return typeveg;
}
