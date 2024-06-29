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
    // Add more icons as needed
  };

  NamedIcon getIcon(String? title) {
    return _icons[title] ?? const NamedIcon(title: 'Default', icon: Icons.help_outline);
  }

  List<NamedIcon> getAllIcons() {
    return _icons.values.toList();
  }
}