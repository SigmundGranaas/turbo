import 'package:flutter/material.dart';
import 'package:turbo/core/widgets/exclusive_sheet.dart';
import 'package:latlong2/latlong.dart';

import 'avalanche_warning_sheet.dart';

Future<void> showAvalancheWarningSheet(
    BuildContext context, LatLng position) {
  return showExclusiveSheet<void>(
    context,
    backgroundColor: Colors.transparent,
    builder: (_) => AvalancheWarningSheet(position: position),
  );
}
