import 'package:flutter/material.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/app/tokens.dart';

import '../models/conditions_source.dart';

/// Chooser listing the available conditions sources for a point. Pops with the
/// chosen [ConditionsSource]; the caller then opens that source's detail.
class ConditionsSheet extends StatelessWidget {
  final List<ConditionsSource> sources;
  final LatLng point;

  const ConditionsSheet({super.key, required this.sources, required this.point});

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: AppSpacing.m),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(
                  AppSpacing.l, AppSpacing.s, AppSpacing.l, AppSpacing.s),
              child: Text('Conditions here', style: textTheme.titleMedium),
            ),
            for (final s in sources)
              ListTile(
                leading: Icon(s.icon),
                title: Text(s.label),
                onTap: () => Navigator.of(context).pop(s),
              ),
          ],
        ),
      ),
    );
  }
}
