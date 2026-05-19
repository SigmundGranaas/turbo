import 'package:flutter/material.dart';
import 'package:latlong2/latlong.dart';

import 'avalanche_warning_sheet.dart';

Future<void> showAvalancheWarningSheet(
    BuildContext context, LatLng position) {
  return showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    showDragHandle: false,
    useSafeArea: true,
    backgroundColor: Colors.transparent,
    builder: (_) => AvalancheWarningSheet(position: position),
  );
}
