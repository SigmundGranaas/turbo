import 'package:latlong2/latlong.dart';

import 'location_service.dart';

/// Tier-classification of a Stedsnavn hit relative to the queried
/// coordinate. The orchestrator uses this to decide whether to prefer
/// the Stedsnavn match outright vs. defer to a protected-area /
/// kommune fallback.
enum LocationMatchTier {
  /// Class 0 — exact contact: on a peak, at a waterline, at a building.
  exactContact,

  /// Class 1 — in a settlement (Tettsted / By / Bygd / Grend / Bydel
  /// within ~1.5 km). Deliberately outranks "close to peak" so that a
  /// pin dropped inside a town surfaces the town name, not a peak
  /// 800 m away.
  inSettlement,

  /// Class 2 — close to a real peak (≤ 1.5 km).
  closeToPeak,

  /// Class 4 — wider periphery: near a settlement (≤ 5 km), close to
  /// a farm / building / water body, or "at" another named feature.
  /// (Class 3 — protected areas — comes from the Vern service.)
  periphery;

  /// `true` when the orchestrator should accept this hit immediately
  /// without consulting the protected-area / kommune fallbacks.
  bool get isTight =>
      this == exactContact ||
      this == inSettlement ||
      this == closeToPeak;
}

/// One scored Stedsnavn match.
class StedsnavnHit {
  final LocationDescription description;
  final LocationMatchTier tier;

  /// Total composite score (`tier-class * 10000 + distance`). Lower
  /// is better. Exposed so the picker can compare across candidates
  /// of the same tier; consumers should look at [tier] / [description]
  /// rather than this raw number.
  final double score;

  const StedsnavnHit({
    required this.description,
    required this.tier,
    required this.score,
  });
}

/// Returns a usable place name from a Kartverket `navn[]` item, or
/// `null` when the entry is anonymous / mislabelled.
///
/// Handles both response shapes this codebase consumes:
///   - `/stedsnavn/v1/navn` (forward search): `skrivemåte` is a top-level
///     string on `navn[i]`.
///   - `/stedsnavn/v1/punkt` (reverse geocode): `skrivemåte` is a string
///     under `navn[i].stedsnavn[0]`. With `navnestatus=hovednavn` the
///     first entry is already the primary spelling, but the helper falls
///     back gracefully if a different shape sneaks through.
///
/// Tolerates the field being missing, empty, whitespace, non-string, or
/// the literal strings "Unknown" / "Ukjent" (Kartverket has been seen
/// returning the latter for some unnamed terrain features).
String? readPlaceName(Map<String, dynamic> item) {
  final candidate = _readSkrivemaate(item['skrivemåte']) ??
      _readFromStedsnavnArray(item['stedsnavn']);
  if (candidate == null || candidate.isEmpty) return null;
  final lc = candidate.toLowerCase();
  if (lc == 'unknown' || lc == 'ukjent') return null;
  return candidate;
}

String? _readSkrivemaate(Object? raw) {
  if (raw is String) {
    final t = raw.trim();
    return t.isEmpty ? null : t;
  }
  if (raw is List && raw.isNotEmpty) {
    // Some endpoints expose `skrivemåte` as a list of language variants.
    for (final v in raw) {
      if (v is String && v.trim().isNotEmpty) return v.trim();
      if (v is Map && v['skrivemåte'] is String) {
        final s = (v['skrivemåte'] as String).trim();
        if (s.isNotEmpty) return s;
      }
    }
  }
  return null;
}

String? _readFromStedsnavnArray(Object? raw) {
  if (raw is! List || raw.isEmpty) return null;
  // Prefer the `hovednavn` entry; fall back to any with a usable
  // skrivemåte. /punkt called with `navnestatus=hovednavn` already
  // filters non-primary spellings out, so this is mostly defensive.
  String? fallback;
  for (final v in raw) {
    if (v is! Map) continue;
    final s = _readSkrivemaate(v['skrivemåte']);
    if (s == null) continue;
    if (v['navnestatus'] == 'hovednavn') return s;
    fallback ??= s;
  }
  return fallback;
}

/// Categorises a Kartverket feature type at a given distance. Returns
/// the tier and the qualifier the UI should render with the name, or
/// `(null, null)` when the candidate should be ignored entirely.
///
/// Distance bands deliberately tight: a pin 1 km from a feature isn't
/// really "there", and showing it as such only fights the kommune
/// fallback. Peripheral matches drop the qualifier word entirely in
/// the UI (see pin_options_sheet `_qualifierLabel`), so we keep them
/// only when they're close enough to feel useful.
(LocationMatchTier?, LocationQualifier?) categorizeFeature(
    String kind, double meters) {
  // Peaks / mountains.
  const peakKinds = {
    'fjelltopp',
    'topp',
    'fjell',
    'ås',
    'haug',
    'berg',
    'nut',
    'pigg',
  };
  // Glaciers and snowfields — you stand ON a glacier, same as a peak.
  const glacierKinds = {'isbre', 'bre', 'snøfonn', 'jøkul'};
  // Lakes, rivers, the sea. "Vatn" is the bokmål/nynorsk variant of
  // "vann" Kartverket sometimes serves up.
  const waterKinds = {
    'innsjø',
    'vann',
    'vatn',
    'tjern',
    'elv',
    'bekk',
    'fjord',
    'sund',
    'vik',
    'bukt',
    'havn',
    'pøl',
  };
  // Islands, islets, headlands — treated like peaks (you're ON the
  // island, not "near" it). Common in Norwegian coastal trekking.
  const landformKinds = {'øy', 'holme', 'skjær', 'nes', 'halvøy'};
  // Settlements — what people call "their town".
  const settlementKinds = {
    'tettsted',
    'by',
    'tettbebyggelse',
    'bygd',
    'grend',
    'bydel',
  };
  // Built features (farms, cabins, single buildings).
  const builtKinds = {'gard', 'bruk', 'seter', 'hytte', 'bygning'};

  // Class 0 — exact contact (you are AT/ON this thing).
  if (peakKinds.contains(kind) && meters <= 100) {
    return (LocationMatchTier.exactContact, LocationQualifier.on);
  }
  if (glacierKinds.contains(kind) && meters <= 200) {
    return (LocationMatchTier.exactContact, LocationQualifier.on);
  }
  if (landformKinds.contains(kind) && meters <= 200) {
    return (LocationMatchTier.exactContact, LocationQualifier.on);
  }
  if (waterKinds.contains(kind) && meters <= 100) {
    return (LocationMatchTier.exactContact, LocationQualifier.atPlace);
  }
  if (builtKinds.contains(kind) && meters <= 50) {
    return (LocationMatchTier.exactContact, LocationQualifier.atPlace);
  }

  // Class 1 — IN a settlement. Tightened from 1.5 km → 800 m: a town
  // centroid that's 1.5 km off is not where the pin actually sits.
  if (settlementKinds.contains(kind) && meters <= 800) {
    return (LocationMatchTier.inSettlement, LocationQualifier.inArea);
  }

  // Class 2 — close to a real peak. Tightened from 1.5 km → 800 m.
  if (peakKinds.contains(kind) && meters <= 800) {
    return (LocationMatchTier.closeToPeak, LocationQualifier.closeTo);
  }

  // Class 4 — wider periphery. Each cap tightened roughly 2× from the
  // previous values; the qualifier word ("close to" / "near") is
  // dropped in the UI anyway, so showing them at 5 km was misleading.
  if (settlementKinds.contains(kind) && meters <= 2000) {
    return (LocationMatchTier.periphery, LocationQualifier.near);
  }
  if (builtKinds.contains(kind) && meters <= 200) {
    return (LocationMatchTier.periphery, LocationQualifier.closeTo);
  }
  if (builtKinds.contains(kind) && meters <= 500) {
    return (LocationMatchTier.periphery, LocationQualifier.near);
  }
  if (waterKinds.contains(kind) && meters <= 500) {
    return (LocationMatchTier.periphery, LocationQualifier.closeTo);
  }
  if (landformKinds.contains(kind) && meters <= 500) {
    return (LocationMatchTier.periphery, LocationQualifier.closeTo);
  }
  if (glacierKinds.contains(kind) && meters <= 500) {
    return (LocationMatchTier.periphery, LocationQualifier.closeTo);
  }
  if (kind.isNotEmpty && meters <= 100) {
    return (LocationMatchTier.periphery, LocationQualifier.atPlace);
  }
  if (kind.isNotEmpty && meters <= 500) {
    return (LocationMatchTier.periphery, LocationQualifier.near);
  }
  return (null, null);
}

/// Builds a [StedsnavnHit] from one `navn[]` entry relative to
/// [queryCoord], or returns `null` when the entry is unusable
/// (anonymous, missing coords, out-of-range feature kind).
StedsnavnHit? describeFeature(LatLng queryCoord, Map<String, dynamic> item) {
  final name = readPlaceName(item);
  if (name == null) return null;
  final pt = item['representasjonspunkt'] as Map<String, dynamic>?;

  // Prefer the server-computed distance from `/punkt`'s response
  // (`meterFraPunkt`). It's the canonical value and lets us skip
  // parsing `representasjonspunkt` for items where it's missing.
  final serverDistance = (item['meterFraPunkt'] as num?)?.toDouble();
  double? distance = serverDistance;
  if (distance == null) {
    if (pt == null) return null;
    final lat = (pt['nord'] as num?)?.toDouble();
    final lng = (pt['øst'] as num?)?.toDouble();
    if (lat == null || lng == null) return null;
    distance = const Distance()
        .as(LengthUnit.Meter, queryCoord, LatLng(lat, lng));
  }

  final kind = (item['navneobjekttype'] as String?)?.toLowerCase() ?? '';
  final (tier, qualifier) = categorizeFeature(kind, distance);
  if (tier == null) return null;

  // Stedsnavn /punkt doesn't return kommuner / fylker on each item,
  // and `navneobjekttype` is redundant with the qualifier ("On" implies
  // a peak/island/glacier, etc.). Leave secondary blank here — the
  // orchestrator enriches the winning description with kommune+fylke
  // from the parallel KommuneBackend call instead.
  final parts = <String>[];
  final kommuner = (item['kommuner'] as List?) ?? const [];
  if (kommuner.isNotEmpty) {
    final k = (kommuner.first as Map?)?['kommunenavn'] as String?;
    if (k != null) parts.add(k);
  }
  final fylker = (item['fylker'] as List?) ?? const [];
  if (fylker.isNotEmpty) {
    final f = (fylker.first as Map?)?['fylkesnavn'] as String?;
    if (f != null) parts.add(f);
  }
  final secondary = parts.isEmpty ? null : parts.join(', ');

  final description = LocationDescription(
    title: name,
    qualifier: qualifier,
    secondary: secondary,
    distanceMeters: distance,
  );
  // Bias the score against inactive / disused features. Kartverket's
  // `stedstatus=hovednavn` filter already drops most of these, but
  // some `stedstatus='historisk'` / `'avslått'` entries still leak
  // through and we'd rather pick a worse-typed live feature over a
  // dead one.
  final status = (item['stedstatus'] as String?)?.toLowerCase() ?? '';
  final statusPenalty = status == 'aktiv' ? 0.0 : 50.0;

  return StedsnavnHit(
    description: description,
    tier: tier,
    score: tier.index * 10000 + distance + statusPenalty,
  );
}
