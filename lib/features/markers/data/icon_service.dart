import 'package:flutter/material.dart';
import 'package:turbo/l10n/app_localizations.dart';

import '../models/named_icon.dart';

class IconService {
  IconService();

  // The keys should be stable and not translated.
  final Map<String, IconData> _icons = {
    'Fjell': Icons.landscape,
    'Park': Icons.park,
    'Strand': Icons.beach_access,
    'Skog': Icons.forest,
    'Vandring': Icons.hiking,
    'Kajakk': Icons.kayaking,
    'Sykkel': Icons.directions_bike,
    'Hytte': Icons.cabin,
    'Parkering': Icons.local_parking,
    'Camping Spot': Icons.airport_shuttle,
    'Badeplass': Icons.pool,
    'Dykking': Icons.scuba_diving,
    'Utkikkspunkt': Icons.photo_camera,
    'Restaurant': Icons.restaurant,
    'Kafé': Icons.local_cafe,
    'Overnatting': Icons.hotel,
    'Fiskeplass': Icons.phishing,
    'Ski': Icons.downhill_skiing,
  };

  String _getLocalizedTitle(BuildContext context, String key) {
    final l10n = context.l10n;
    switch (key) {
      case 'Fjell': return l10n.iconFjell;
      case 'Park': return l10n.iconPark;
      case 'Strand': return l10n.iconStrand;
      case 'Skog': return l10n.iconSkog;
      case 'Vandring': return l10n.iconVandring;
      case 'Kajakk': return l10n.iconKajakk;
      case 'Sykkel': return l10n.iconSykkel;
      case 'Hytte': return l10n.iconHytte;
      case 'Parkering': return l10n.iconParkering;
      case 'Camping Spot': return l10n.iconCampingSpot;
      case 'Badeplass': return l10n.iconBadeplass;
      case 'Dykking': return l10n.iconDykking;
      case 'Utkikkspunkt': return l10n.iconUtkikkspunkt;
      case 'Restaurant': return l10n.iconRestaurant;
      case 'Kafé': return l10n.iconKafe;
      case 'Overnatting': return l10n.iconOvernatting;
      case 'Fiskeplass': return l10n.iconFiskeplass;
      case 'Ski': return l10n.iconSki;
      default: return l10n.iconDefault;
    }
  }

  NamedIcon getIcon(BuildContext context, String? key) {
    final l10n = context.l10n;
    final iconData = _icons[key] ?? Icons.help_outline;
    final title = key != null ? _getLocalizedTitle(context, key) : l10n.iconDefault;

    // The NamedIcon's title should be the key, but we can construct it with a localized title for display.
    // Let's adjust NamedIcon to hold both the key and the display name. For now, we'll use the key as the title.
    return NamedIcon(
      icon: iconData,
      title: key ?? 'Default', // This is the stable key
      localizedTitle: title, // This is for display
    );
  }

  List<NamedIcon> getAllIcons(BuildContext context) {
    return _icons.entries.map((entry) {
      return NamedIcon(
        icon: entry.value,
        title: entry.key, // The key
        localizedTitle: _getLocalizedTitle(context, entry.key), // The display name
      );
    }).toList();
  }

  // A default icon for when no key is provided.
  NamedIcon getDefaultIcon(BuildContext context) {
    return NamedIcon(
      icon: Icons.help_outline,
      title: 'Default',
      localizedTitle: context.l10n.iconDefault,
    );
  }
}