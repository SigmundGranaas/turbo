import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/features/activities/api.dart';

import '../data/packrafting_repository.dart';
import '../models/packrafting_details.dart';

class PackraftingCreateScreen extends ConsumerStatefulWidget {
  final ActivityGeometry seedGeometry;
  const PackraftingCreateScreen({super.key, required this.seedGeometry});

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
  void dispose() {
    _name.dispose(); _description.dispose();
    _distance.dispose(); _paddle.dispose(); _portage.dispose();
    _minFlow.dispose(); _maxFlow.dispose(); _nve.dispose();
    super.dispose();
  }

  LatLng get _seed => widget.seedGeometry.firstPoint ?? const LatLng(0, 0);
  List<LatLng> get _route => [_seed, LatLng(_seed.latitude, _seed.longitude + 0.001)];

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('New packrafting trip')),
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
            Expanded(child: _num(_distance, 'Distance m', required: true)),
            const SizedBox(width: 8),
            Expanded(child: _num(_paddle, 'Paddle m', required: true)),
            const SizedBox(width: 8),
            Expanded(child: _num(_portage, 'Portage m', required: true)),
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
            Expanded(child: _num(_minFlow, 'Min flow m³/s', required: false)),
            const SizedBox(width: 8),
            Expanded(child: _num(_maxFlow, 'Max flow m³/s', required: false)),
          ]),
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

  Widget _num(TextEditingController c, String label, {required bool required}) => TextFormField(
    controller: c, keyboardType: TextInputType.number,
    decoration: InputDecoration(labelText: label, border: const OutlineInputBorder(), isDense: true),
    validator: required
      ? (v) => double.tryParse(v ?? '') == null ? 'Number required' : null
      : (v) => (v == null || v.isEmpty) ? null : (double.tryParse(v) == null ? 'Number' : null));

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
        putIn: _seed, takeOut: _route.last,
        nveStationCode: _nve.text.trim().isEmpty ? null : _nve.text.trim(),
        minFlowCumecs: double.tryParse(_minFlow.text),
        maxFlowCumecs: double.tryParse(_maxFlow.text),
      );
      await ref.read(packraftingRepositoryProvider).create(
        name: _name.text.trim(),
        description: _description.text.trim().isEmpty ? null : _description.text.trim(),
        route: _route, details: details);
      if (mounted) Navigator.of(context).pop();
    } catch (e) {
      if (mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Failed: $e')));
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }
}
