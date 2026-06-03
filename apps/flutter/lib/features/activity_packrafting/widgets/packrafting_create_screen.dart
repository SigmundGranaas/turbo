import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/features/activities/api.dart';

import '../data/packrafting_repository.dart';
import '../models/packrafting_activity.dart';
import '../models/packrafting_details.dart';

class PackraftingCreateScreen extends ConsumerStatefulWidget {
  final ActivityGeometry seedGeometry;

  /// If non-null the form opens in edit mode: fields and the drawn
  /// route are prefilled from this activity and Save calls `update`.
  final PackraftingActivity? existing;

  const PackraftingCreateScreen({
    super.key,
    required this.seedGeometry,
    this.existing,
  });

  @override
  ConsumerState<PackraftingCreateScreen> createState() => _PackraftingCreateScreenState();
}

class _PackraftingCreateScreenState extends ConsumerState<PackraftingCreateScreen> {
  final _formKey = GlobalKey<FormState>();
  final _name = TextEditingController();
  final _description = TextEditingController();
  final _distance = TextEditingController(text: '10000');
  final _paddle = TextEditingController(text: '8000');
  final _portage = TextEditingController(text: '2000');
  final _minFlow = TextEditingController();
  final _maxFlow = TextEditingController();
  final _nve = TextEditingController();
  WaterGrade _typical = WaterGrade.ii;
  WaterGrade _max = WaterGrade.iii;
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    final e = widget.existing;
    if (e != null) {
      _name.text = e.name;
      _description.text = e.description ?? '';
      _distance.text = e.details.distanceMeters.toString();
      _paddle.text = e.details.paddleDistanceMeters.toString();
      _portage.text = e.details.portageDistanceMeters.toString();
      _minFlow.text = e.details.minFlowCumecs?.toString() ?? '';
      _maxFlow.text = e.details.maxFlowCumecs?.toString() ?? '';
      _nve.text = e.details.nveStationCode ?? '';
      _typical = e.details.typicalGrade;
      _max = e.details.maxGrade;
      _drawnRoute = List<LatLng>.from(e.route);
    }
  }

  @override
  void dispose() {
    _name.dispose(); _description.dispose();
    _distance.dispose(); _paddle.dispose(); _portage.dispose();
    _minFlow.dispose(); _maxFlow.dispose(); _nve.dispose();
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
    return ActivityCreateScaffold(
      title: _isEdit ? 'Edit packrafting trip' : 'New packrafting trip',
      formKey: _formKey,
      nameController: _name,
      descriptionController: _description,
      saving: _saving,
      onSave: _save,
      routeButton: OutlinedButton.icon(
        onPressed: _saving ? null : _drawRoute,
        icon: const Icon(Icons.timeline_outlined),
        label: Text(_drawnRoute == null
            ? 'Draw river course on map'
            : 'Drawn: ${_drawnRoute!.length} pts · ${(routeDistanceMeters(_drawnRoute!) / 1000).toStringAsFixed(2)} km'),
      ),
      fields: [
        Row(children: [
          Expanded(child: ActivityCreateScaffold.numberField(_distance, 'Distance m')),
          const SizedBox(width: 8),
          Expanded(child: ActivityCreateScaffold.numberField(_paddle, 'Paddle m')),
          const SizedBox(width: 8),
          Expanded(child: ActivityCreateScaffold.numberField(_portage, 'Portage m')),
        ]),
        const SizedBox(height: 16),
        Text('Typical grade', style: Theme.of(context).textTheme.labelLarge),
        const SizedBox(height: 4),
        DropdownButtonFormField<WaterGrade>(
          initialValue: _typical,
          decoration: const InputDecoration(border: OutlineInputBorder()),
          items: WaterGrade.values.map((g) =>
            DropdownMenuItem(value: g, child: Text(_gradeLabel(g)))).toList(),
          onChanged: (v) => setState(() => _typical = v ?? WaterGrade.ii)),
        const SizedBox(height: 8),
        Text('Max grade', style: Theme.of(context).textTheme.labelLarge),
        const SizedBox(height: 4),
        DropdownButtonFormField<WaterGrade>(
          initialValue: _max,
          decoration: const InputDecoration(border: OutlineInputBorder()),
          items: WaterGrade.values.map((g) =>
            DropdownMenuItem(value: g, child: Text(_gradeLabel(g)))).toList(),
          onChanged: (v) => setState(() => _max = v ?? WaterGrade.iii)),
        const SizedBox(height: 16),
        TextFormField(controller: _nve,
          decoration: const InputDecoration(labelText: 'NVE station code (optional)', border: OutlineInputBorder())),
        const SizedBox(height: 8),
        Row(children: [
          Expanded(child: ActivityCreateScaffold.numberField(_minFlow, 'Min flow m³/s', required: false)),
          const SizedBox(width: 8),
          Expanded(child: ActivityCreateScaffold.numberField(_maxFlow, 'Max flow m³/s', required: false)),
        ]),
      ],
    );
  }

  static String _gradeLabel(WaterGrade g) => switch (g) {
    WaterGrade.flatwater => 'Flatwater',
    WaterGrade.i => 'I',
    WaterGrade.ii => 'II',
    WaterGrade.iii => 'III',
    WaterGrade.iv => 'IV',
    WaterGrade.v => 'V',
    WaterGrade.vi => 'VI (unrunnable)',
  };

  Future<void> _save() async {
    if (!(_formKey.currentState?.validate() ?? false)) return;
    if (_typical.index > _max.index) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Typical grade must not exceed max grade')));
      return;
    }
    setState(() => _saving = true);
    try {
      final details = PackraftingDetails(
        distanceMeters: int.parse(_distance.text),
        paddleDistanceMeters: int.parse(_paddle.text),
        portageDistanceMeters: int.parse(_portage.text),
        maxGrade: _max, typicalGrade: _typical,
        putIn: _route.first, takeOut: _route.last,
        nveStationCode: _nve.text.trim().isEmpty ? null : _nve.text.trim(),
        minFlowCumecs: double.tryParse(_minFlow.text),
        maxFlowCumecs: double.tryParse(_maxFlow.text),
      );
      final repo = ref.read(packraftingRepositoryProvider);
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
