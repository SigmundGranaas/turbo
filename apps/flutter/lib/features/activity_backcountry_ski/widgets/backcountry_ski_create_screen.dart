import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/features/activities/api.dart';

import '../data/backcountry_ski_repository.dart';
import '../models/backcountry_ski_activity.dart';
import '../models/backcountry_ski_details.dart';

/// Stub create/edit screen. The shell's picker hands us a seed point at the
/// long-press location; in this minimal v1 we let the user save a name +
/// numeric stats and persist a synthetic 1-segment route around that
/// seed (or the existing route on edit).
class BackcountrySkiCreateScreen extends ConsumerStatefulWidget {
  final ActivityGeometry seedGeometry;

  /// If non-null the form opens in edit mode: fields and the drawn
  /// route are prefilled from this activity and Save calls `update`.
  final BackcountrySkiActivity? existing;

  const BackcountrySkiCreateScreen({
    super.key,
    required this.seedGeometry,
    this.existing,
  });

  @override
  ConsumerState<BackcountrySkiCreateScreen> createState() =>
      _BackcountrySkiCreateScreenState();
}

class _BackcountrySkiCreateScreenState
    extends ConsumerState<BackcountrySkiCreateScreen> {
  final _formKey = GlobalKey<FormState>();
  final _name = TextEditingController();
  final _description = TextEditingController();
  final _ascent = TextEditingController(text: '800');
  final _descent = TextEditingController(text: '800');
  final _distance = TextEditingController(text: '6500');
  final _elevMin = TextEditingController(text: '900');
  final _elevMax = TextEditingController(text: '1700');
  AtesRating _ates = AtesRating.challenging;
  Aspect _aspect = Aspect.n;
  int? _preferredAvalancheMaxLevel;
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    final e = widget.existing;
    if (e != null) {
      _name.text = e.name;
      _description.text = e.description ?? '';
      _ascent.text = e.details.ascentMeters.toString();
      _descent.text = e.details.descentMeters.toString();
      _distance.text = e.details.distanceMeters.toString();
      _elevMin.text = e.details.elevationMinMeters.toString();
      _elevMax.text = e.details.elevationMaxMeters.toString();
      _ates = e.details.atesRating;
      _aspect = e.details.dominantAspect ?? Aspect.n;
      _preferredAvalancheMaxLevel = e.details.preferredAvalancheMaxLevel;
      _drawnRoute = List<LatLng>.from(e.route);
    }
  }

  @override
  void dispose() {
    _name.dispose();
    _description.dispose();
    _ascent.dispose();
    _descent.dispose();
    _distance.dispose();
    _elevMin.dispose();
    _elevMax.dispose();
    super.dispose();
  }

  bool get _isEdit => widget.existing != null;

  LatLng get _seed => widget.seedGeometry.firstPoint ?? const LatLng(0, 0);

  List<LatLng>? _drawnRoute;

  Future<void> _drawRoute() async {
    final route = await Navigator.of(context).push<List<LatLng>>(
      MaterialPageRoute(
        builder: (ctx) => RouteDrawingScreen(
          seedCenter: _drawnRoute?.first ?? _seed,
          initialRoute: _drawnRoute,
          color: const Color(0xFF5E72A5),
        ),
      ),
    );
    if (route != null && route.length >= 2) {
      setState(() => _drawnRoute = route);
    }
  }

  /// Drawn route when the user has tapped one out; otherwise a minimal
  /// 1-segment synthetic placeholder. The aggregate validates that the
  /// stored details (distance, ascent, etc.) are non-negative; nothing
  /// requires them to match the geometry.
  List<LatLng> get _route => _drawnRoute ?? [
        _seed,
        LatLng(_seed.latitude, _seed.longitude + 0.0005),
      ];

  @override
  Widget build(BuildContext context) {
    return ActivityCreateScaffold(
      title: _isEdit ? 'Edit backcountry route' : 'New backcountry route',
      formKey: _formKey,
      nameController: _name,
      descriptionController: _description,
      saving: _saving,
      onSave: _save,
      routeButton: OutlinedButton.icon(
        onPressed: _saving ? null : _drawRoute,
        icon: const Icon(Icons.timeline_outlined),
        label: Text(_drawnRoute == null
            ? 'Draw route on map'
            : 'Drawn: ${_drawnRoute!.length} pts · ${(routeDistanceMeters(_drawnRoute!) / 1000).toStringAsFixed(2)} km'),
      ),
      fields: [
        Row(
          children: [
            Expanded(child: ActivityCreateScaffold.numberField(_ascent, 'Ascent m')),
            const SizedBox(width: 8),
            Expanded(child: ActivityCreateScaffold.numberField(_descent, 'Descent m')),
          ],
        ),
        const SizedBox(height: 8),
        Row(
          children: [
            Expanded(child: ActivityCreateScaffold.numberField(_distance, 'Distance m')),
            const SizedBox(width: 8),
            Expanded(child: ActivityCreateScaffold.numberField(_elevMin, 'Min elev m')),
            const SizedBox(width: 8),
            Expanded(child: ActivityCreateScaffold.numberField(_elevMax, 'Max elev m')),
          ],
        ),
        const SizedBox(height: 16),
        Text('ATES', style: Theme.of(context).textTheme.labelLarge),
        const SizedBox(height: 4),
        // `AtesRating.unrated` exists on the model — leaving it out of the
        // segment list crashes SegmentedButton when editing a legacy activity
        // whose rating is unrated.
        SegmentedButton<AtesRating>(
          segments: const [
            ButtonSegment(value: AtesRating.unrated, label: Text('Unrated')),
            ButtonSegment(value: AtesRating.simple, label: Text('Simple')),
            ButtonSegment(value: AtesRating.challenging, label: Text('Challenging')),
            ButtonSegment(value: AtesRating.complex, label: Text('Complex')),
          ],
          selected: {_ates},
          onSelectionChanged: (s) => setState(() => _ates = s.first),
        ),
        const SizedBox(height: 16),
        Text('Dominant aspect', style: Theme.of(context).textTheme.labelLarge),
        const SizedBox(height: 4),
        DropdownButtonFormField<Aspect>(
          initialValue: _aspect,
          decoration: const InputDecoration(border: OutlineInputBorder()),
          items: Aspect.values
              .map((a) =>
                  DropdownMenuItem(value: a, child: Text(a.name.toUpperCase())))
              .toList(),
          onChanged: (v) => setState(() => _aspect = v ?? Aspect.n),
        ),
        const SizedBox(height: 16),
        Text('Max acceptable avalanche danger',
            style: Theme.of(context).textTheme.labelLarge),
        const SizedBox(height: 4),
        Text(
          'Used to flag the route when the Varsom forecast exceeds this level.',
          style: Theme.of(context).textTheme.bodySmall,
        ),
        const SizedBox(height: 4),
        DropdownButtonFormField<int?>(
          initialValue: _preferredAvalancheMaxLevel,
          decoration: const InputDecoration(border: OutlineInputBorder()),
          items: const [
            DropdownMenuItem<int?>(value: null, child: Text('Any')),
            DropdownMenuItem<int?>(value: 1, child: Text('1 — Low')),
            DropdownMenuItem<int?>(value: 2, child: Text('2 — Moderate')),
            DropdownMenuItem<int?>(value: 3, child: Text('3 — Considerable')),
            DropdownMenuItem<int?>(value: 4, child: Text('4 — High')),
            DropdownMenuItem<int?>(value: 5, child: Text('5 — Extreme')),
          ],
          onChanged: (v) => setState(() => _preferredAvalancheMaxLevel = v),
        ),
        const SizedBox(height: 12),
        ListTile(
          leading: const Icon(Icons.flag_outlined),
          title: const Text('Start'),
          subtitle: Text(
            '${_seed.latitude.toStringAsFixed(5)}, '
            '${_seed.longitude.toStringAsFixed(5)}',
          ),
          contentPadding: EdgeInsets.zero,
        ),
      ],
    );
  }

  Future<void> _save() async {
    if (!(_formKey.currentState?.validate() ?? false)) return;
    setState(() => _saving = true);
    try {
      final details = BackcountrySkiDetails(
        ascentMeters: int.parse(_ascent.text),
        descentMeters: int.parse(_descent.text),
        distanceMeters: int.parse(_distance.text),
        elevationMinMeters: int.parse(_elevMin.text),
        elevationMaxMeters: int.parse(_elevMax.text),
        atesRating: _ates,
        dominantAspect: _aspect,
        preferredAvalancheMaxLevel: _preferredAvalancheMaxLevel,
      );
      final repo = ref.read(backcountrySkiRepositoryProvider);
      final existing = widget.existing;
      if (existing != null) {
        await repo.update(
          id: existing.id,
          name: _name.text.trim(),
          description: _description.text.trim().isEmpty
              ? null
              : _description.text.trim(),
          route: _route,
          details: details,
        );
      } else {
        await repo.create(
              name: _name.text.trim(),
              description: _description.text.trim().isEmpty
                  ? null
                  : _description.text.trim(),
              route: _route,
              details: details,
            );
      }
      if (mounted) Navigator.of(context).pop(true);
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text('Failed to save: $e')));
      }
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }
}
