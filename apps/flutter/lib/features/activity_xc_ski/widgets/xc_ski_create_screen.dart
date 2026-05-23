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
          color: const Color(0xFF00838F),
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
    return Scaffold(
      appBar: AppBar(title: Text(_isEdit ? 'Edit XC ski trail' : 'New XC ski trail')),
      body: SafeArea(child: Form(key: _formKey, child: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          TextFormField(controller: _name,
            decoration: const InputDecoration(labelText: 'Name', border: OutlineInputBorder()),
            validator: (v) => (v?.trim().isEmpty ?? true) ? 'Required' : null),
          const SizedBox(height: 12),
          TextFormField(controller: _description, maxLines: 2,
            decoration: const InputDecoration(labelText: 'Description (optional)', border: OutlineInputBorder())),
          const SizedBox(height: 16),
          Row(children: [
            Expanded(child: _num(_distance, 'Distance m')),
            const SizedBox(width: 8),
            Expanded(child: _num(_ascent, 'Ascent m')),
            const SizedBox(width: 8),
            Expanded(child: _num(_descent, 'Descent m')),
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
          const SizedBox(height: 16),
          OutlinedButton.icon(
            onPressed: _saving ? null : _drawRoute,
            icon: const Icon(Icons.timeline_outlined),
            label: Text(_drawnRoute == null
                ? 'Draw trail on map'
                : 'Drawn: ${_drawnRoute!.length} pts · ${(routeDistanceMeters(_drawnRoute!) / 1000).toStringAsFixed(2)} km'),
          ),
          const SizedBox(height: 24),
          FilledButton.icon(
            onPressed: _saving ? null : _save,
            icon: _saving
              ? const SizedBox(width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2))
              : const Icon(Icons.check),
            label: const Text('Save')),
        ],
      ))),
    );
  }

  Widget _num(TextEditingController c, String label) => TextFormField(
    controller: c, keyboardType: TextInputType.number,
    decoration: InputDecoration(labelText: label, border: const OutlineInputBorder(), isDense: true),
    validator: (v) => int.tryParse(v ?? '') == null ? 'Number required' : null);

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
      if (mounted) Navigator.of(context).pop();
    } catch (e) {
      if (mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Failed: $e')));
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }
}
