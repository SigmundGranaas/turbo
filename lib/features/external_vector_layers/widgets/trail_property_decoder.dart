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

  /// Project [feature]'s raw properties through the SOSI code tables.
  factory TrailProperties.from(VectorFeature feature, BuildContext context) {
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
