import 'package:flutter/cupertino.dart';

class NamedIcon {
  final String title; // This is the stable, non-translated key. e.g., "Fjell"
  final String? localizedTitle; // This is the translated name for display. e.g., "Mountain"
  final IconData icon;

  const NamedIcon({
    required this.icon,
    required this.title,
    this.localizedTitle,
  });
}