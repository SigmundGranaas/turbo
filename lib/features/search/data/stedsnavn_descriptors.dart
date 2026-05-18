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
/// `null` when the entry is anonymous / mislabelled. Tolerates the
/// field being missing, empty, whitespace, non-string, or the literal
/// strings "Unknown" / "Ukjent" (Kartverket has been seen returning
/// the latter for some unnamed terrain features).
String? readPlaceName(Map<String, dynamic> item) {
  final raw = item['skrivemåte'];
  String? candidate;
  if (raw is String) {
    candidate = raw.trim();
  } else if (raw is List && raw.isNotEmpty) {
    // Future-proof: some Kartverket endpoints expose `skrivemåte` as a
    // list of language variants. Take the first non-empty entry.
    for (final v in raw) {
      if (v is String && v.trim().isNotEmpty) {
        candidate = v.trim();
        break;
      }
      if (v is Map && v['skrivemåte'] is String) {
        final s = (v['skrivemåte'] as String).trim();
        if (s.isNotEmpty) {
          candidate = s;
          break;
        }
      }
    }
  }
  if (candidate == null || candidate.isEmpty) return null;
  final lc = candidate.toLowerCase();
  if (lc == 'unknown' || lc == 'ukjent') return null;
  return candidate;
}

/// Categorises a Kartverket feature type at a given distance. Returns
/// the tier and the qualifier the UI should render with the name, or
/// `(null, null)` when the candidate should be ignored entirely
/// (e.g. a `haug` 5 km away).
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
  // Water features.
  const waterKinds = {'innsjø', 'vann', 'elv', 'fjord', 'bekk', 'tjern'};
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

  // Class 0 — exact contact (you are AT this thing).
  if (peakKinds.contains(kind) && meters <= 100) {
    return (LocationMatchTier.exactContact, LocationQualifier.on);
  }
  if (waterKinds.contains(kind) && meters <= 100) {
    return (LocationMatchTier.exactContact, LocationQualifier.atPlace);
  }
  if (builtKinds.contains(kind) && meters <= 50) {
    return (LocationMatchTier.exactContact, LocationQualifier.atPlace);
  }

  // Class 1 — IN a settlement.
  if (settlementKinds.contains(kind) && meters <= 1500) {
    return (LocationMatchTier.inSettlement, LocationQualifier.inArea);
  }

  // Class 2 — close to a real peak.
  if (peakKinds.contains(kind) && meters <= 1500) {
    return (LocationMatchTier.closeToPeak, LocationQualifier.closeTo);
  }

  // Class 4 — wider periphery.
  if (settlementKinds.contains(kind) && meters <= 5000) {
    return (LocationMatchTier.periphery, LocationQualifier.near);
  }
  if (builtKinds.contains(kind) && meters <= 500) {
    return (LocationMatchTier.periphery, LocationQualifier.closeTo);
  }
  if (builtKinds.contains(kind) && meters <= 2000) {
    return (LocationMatchTier.periphery, LocationQualifier.near);
  }
  if (waterKinds.contains(kind) && meters <= 1000) {
    return (LocationMatchTier.periphery, LocationQualifier.closeTo);
  }
  if (kind.isNotEmpty && meters <= 100) {
    return (LocationMatchTier.periphery, LocationQualifier.atPlace);
  }
  if (kind.isNotEmpty && meters <= 1000) {
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
  if (pt == null) return null;
  final lat = (pt['nord'] as num?)?.toDouble();
  final lng = (pt['øst'] as num?)?.toDouble();
  if (lat == null || lng == null) return null;
  final distance = const Distance()
      .as(LengthUnit.Meter, queryCoord, LatLng(lat, lng));
  final kind = (item['navneobjekttype'] as String?)?.toLowerCase() ?? '';
  final (tier, qualifier) = categorizeFeature(kind, distance);
  if (tier == null) return null;

  // Build the human-readable secondary line ("Fjell, Lom, Innlandet").
  final parts = <String>[];
  final objectType = item['navneobjekttype'] as String?;
  if (objectType != null && objectType.isNotEmpty) parts.add(objectType);
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
  return StedsnavnHit(
    description: description,
    tier: tier,
    score: tier.index * 10000 + distance,
  );
}
