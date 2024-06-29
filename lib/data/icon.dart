import 'package:flutter/material.dart';

import 'model/named_icon.dart';


class IconService {
  static final IconService _instance = IconService._internal();

  factory IconService() {
    return _instance;
  }

  IconService._internal();

  final Map<String, NamedIcon> _icons = {
    'Fjell': const NamedIcon(title: 'Fjell', icon: Icons.landscape),
    'Park': const NamedIcon(title: 'Park', icon: Icons.park),
    'Strand': const NamedIcon(title: 'Strand', icon: Icons.beach_access),
    'Skog': const NamedIcon(title: 'Skog', icon: Icons.forest),
    'Vandring': const NamedIcon(title: 'Vandring', icon: Icons.hiking),
    'Kajakk': const NamedIcon(title: 'Kajakk', icon: Icons.kayaking),
    'Sykkel': const NamedIcon(title: 'Sykkel', icon: Icons.directions_bike),
    'Hytte': const NamedIcon(title: 'Hytte', icon: Icons.cabin),
    'Parkering': const NamedIcon(title: 'Parkering', icon: Icons.local_parking),
    'Camping Spot': const NamedIcon(title: 'Camping Spot', icon: Icons.airport_shuttle),
    'Badeplass': const NamedIcon(title: 'Badeplass', icon: Icons.pool),
    'Dykking': const NamedIcon(title: 'Dykking', icon: Icons.scuba_diving),
    'Utkikkspunkt': const NamedIcon(title: 'Utkikkspunkt', icon: Icons.photo_camera),
    'Restaurant': const NamedIcon(title: 'Restaurant', icon: Icons.restaurant),
    'Kafé': const NamedIcon(title: 'Kafé', icon: Icons.local_cafe),
    'Overnatting': const NamedIcon(title: 'Overnatting', icon: Icons.hotel),
    'Fiskeplass': const NamedIcon(title: 'Fiskeplass', icon: Icons.phishing),
    'Ski': const NamedIcon(title: 'Ski', icon: Icons.downhill_skiing),
    // Add more icons as needed
  };

  NamedIcon getIcon(String? title) {
    return _icons[title] ?? const NamedIcon(title: 'Default', icon: Icons.help_outline);
  }

  List<NamedIcon> getAllIcons() {
    return _icons.values.toList();
  }
}