import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/features/activities/api.dart';

import '../data/xc_ski_repository.dart';
import '../models/xc_ski_activity.dart';
import '../models/xc_ski_details.dart';

class XcSkiCreateScreen extends ConsumerStatefulWidget {
  final ActivityGeometry seedGeometry;

  /// If non-null the form opens in edit mode: fields and the drawn
  /// route are prefilled from this activity and Save calls `update`.
  final XcSkiActivity? existing;

  const XcSkiCreateScreen({
    super.key,
    required this.seedGeometry,
    this.existing,
  });

  @override
  ConsumerState<XcSkiCreateScreen> createState() => _XcSkiCreateScreenState();
}

class _XcSkiCreateScreenState extends ConsumerState<XcSkiCreateScreen> {
  final _formKey = GlobalKey<FormState>();
  final _name = TextEditingController();
  final _description = TextEditingController();
  final _distance = TextEditingController(text: '5000');
  final _ascent = TextEditingController(text: '50');
  final _descent = TextEditingController(text: '50');
  XcSkiTechnique _technique = XcSkiTechnique.both;
  GroomingStatus _grooming = GroomingStatus.unknown;
  bool _lit = false;
  bool _seasonPass = false;
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    final e = widget.existing;
    if (e != null) {
      _name.text = e.name;
      _description.text = e.description ?? '';
      _distance.text = e.details.distanceMeters.toString();
      _ascent.text = e.details.ascentMeters.toString();
      _descent.text = e.details.descentMeters.toString();
      _technique = e.details.technique;
      _grooming = e.details.groomingStatus;
      _lit = e.details.isLit;
      _seasonPass = e.details.requiresSeasonPass;
      _drawnRoute = List<LatLng>.from(e.route);
    }
  }

  @override
  void dispose() {
    _name.dispose(); _description.dispose();
    _distance.dispose(); _ascent.dispose(); _descent.dispose();
    super.dispose();
  }

  bool get _isEdit => widget.existing != null;

  LatLng get _seed => widget.seedGeometry.firstPoint ?? const LatLng(0, 0);
  List<LatLng>? _drawnRoute;
  List<LatLng> get _route =>
      _drawnRoute ?? [_seed, LatLng(_seed.latitude, _seed.longitude + 0.001)];

  Future<void> _drawRoute() async {
    final route = await Navigator.of(context).push<List<LatLng>>(
      MaterialPageRoute(
        builder: (ctx) => RouteDrawingScreen(
          seedCenter: _drawnRoute?.first ?? _seed,
          initialRoute: _drawnRoute,
          color: const Color(0xFF0288D1),
        ),
      ),
    );
    if (route != null && route.length >= 2) {
      setState(() {
        _drawnRoute = route;
        _distance.text = routeDistanceMeters(route).round().toString();
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return ActivityCreateScaffold(
      title: _isEdit ? 'Edit XC ski trail' : 'New XC ski trail',
      formKey: _formKey,
      nameController: _name,
      descriptionController: _description,
      saving: _saving,
      onSave: _save,
      routeButton: OutlinedButton.icon(
        onPressed: _saving ? null : _drawRoute,
        icon: const Icon(Icons.timeline_outlined),
        label: Text(_drawnRoute == null
            ? 'Draw trail on map'
            : 'Drawn: ${_drawnRoute!.length} pts · ${(routeDistanceMeters(_drawnRoute!) / 1000).toStringAsFixed(2)} km'),
      ),
      fields: [
        Row(children: [
          Expanded(child: ActivityCreateScaffold.numberField(_distance, 'Distance m')),
          const SizedBox(width: 8),
          Expanded(child: ActivityCreateScaffold.numberField(_ascent, 'Ascent m')),
          const SizedBox(width: 8),
          Expanded(child: ActivityCreateScaffold.numberField(_descent, 'Descent m')),
        ]),
        const SizedBox(height: 16),
        Text('Technique', style: Theme.of(context).textTheme.labelLarge),
        const SizedBox(height: 4),
        SegmentedButton<XcSkiTechnique>(
          segments: const [
            ButtonSegment(value: XcSkiTechnique.classic, label: Text('Classic')),
            ButtonSegment(value: XcSkiTechnique.skate, label: Text('Skate')),
            ButtonSegment(value: XcSkiTechnique.both, label: Text('Both')),
            ButtonSegment(value: XcSkiTechnique.backcountry, label: Text('BC')),
          ],
          selected: {_technique},
          onSelectionChanged: (s) => setState(() => _technique = s.first)),
        const SizedBox(height: 12),
        DropdownButtonFormField<GroomingStatus>(
          initialValue: _grooming,
          decoration: const InputDecoration(labelText: 'Grooming', border: OutlineInputBorder()),
          items: GroomingStatus.values.map((g) =>
            DropdownMenuItem(value: g, child: Text(g.name))).toList(),
          onChanged: (v) => setState(() => _grooming = v ?? GroomingStatus.unknown)),
        const SizedBox(height: 8),
        SwitchListTile(value: _lit, onChanged: (v) => setState(() => _lit = v),
          title: const Text('Lit trail'), contentPadding: EdgeInsets.zero),
        SwitchListTile(value: _seasonPass, onChanged: (v) => setState(() => _seasonPass = v),
          title: const Text('Requires season pass'), contentPadding: EdgeInsets.zero),
      ],
    );
  }

  Future<void> _save() async {
    if (!(_formKey.currentState?.validate() ?? false)) return;
    setState(() => _saving = true);
    try {
      final details = XcSkiDetails(
        distanceMeters: int.parse(_distance.text),
        ascentMeters: int.parse(_ascent.text),
        descentMeters: int.parse(_descent.text),
        technique: _technique, groomingStatus: _grooming,
        isLit: _lit, requiresSeasonPass: _seasonPass);
      final repo = ref.read(xcSkiRepositoryProvider);
      final existing = widget.existing;
      if (existing != null) {
        await repo.update(
          id: existing.id,
          name: _name.text.trim(),
          description: _description.text.trim().isEmpty ? null : _description.text.trim(),
          route: _route, details: details);
      } else {
        await repo.create(
          name: _name.text.trim(),
          description: _description.text.trim().isEmpty ? null : _description.text.trim(),
          route: _route, details: details);
      }
      if (mounted) Navigator.of(context).pop(true);
    } catch (e) {
      if (mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Failed: $e')));
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }
}
