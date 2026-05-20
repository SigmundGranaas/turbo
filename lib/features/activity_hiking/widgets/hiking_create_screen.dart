import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/features/activities/api.dart';

import '../data/hiking_repository.dart';
import '../models/hiking_details.dart';

class HikingCreateScreen extends ConsumerStatefulWidget {
  final ActivityGeometry seedGeometry;
  const HikingCreateScreen({super.key, required this.seedGeometry});

  @override
  ConsumerState<HikingCreateScreen> createState() => _HikingCreateScreenState();
}

class _HikingCreateScreenState extends ConsumerState<HikingCreateScreen> {
  final _formKey = GlobalKey<FormState>();
  final _name = TextEditingController();
  final _description = TextEditingController();
  final _distance = TextEditingController(text: '8000');
  final _ascent = TextEditingController(text: '500');
  final _descent = TextEditingController(text: '500');
  final _elevMin = TextEditingController(text: '500');
  final _elevMax = TextEditingController(text: '1100');
  final _hours = TextEditingController(text: '4');
  HikingDifficulty _difficulty = HikingDifficulty.moderate;
  TrailSurface _surface = TrailSurface.path;
  TrailMarking _marking = TrailMarking.signposted;
  bool _water = true;
  bool _shelter = false;
  bool _saving = false;

  @override
  void dispose() {
    _name.dispose(); _description.dispose();
    _distance.dispose(); _ascent.dispose(); _descent.dispose();
    _elevMin.dispose(); _elevMax.dispose(); _hours.dispose();
    super.dispose();
  }

  LatLng get _seed => widget.seedGeometry.firstPoint ?? const LatLng(0, 0);
  List<LatLng> get _route => [_seed, LatLng(_seed.latitude, _seed.longitude + 0.001)];

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('New hike')),
      body: SafeArea(
        child: Form(
          key: _formKey,
          child: ListView(
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
                Expanded(child: _num(_hours, 'Hours')),
              ]),
              const SizedBox(height: 8),
              Row(children: [
                Expanded(child: _num(_ascent, 'Ascent m')),
                const SizedBox(width: 8),
                Expanded(child: _num(_descent, 'Descent m')),
              ]),
              const SizedBox(height: 8),
              Row(children: [
                Expanded(child: _num(_elevMin, 'Min elev m')),
                const SizedBox(width: 8),
                Expanded(child: _num(_elevMax, 'Max elev m')),
              ]),
              const SizedBox(height: 16),
              Text('Difficulty', style: Theme.of(context).textTheme.labelLarge),
              const SizedBox(height: 4),
              SegmentedButton<HikingDifficulty>(
                segments: const [
                  ButtonSegment(value: HikingDifficulty.easy, label: Text('Easy')),
                  ButtonSegment(value: HikingDifficulty.moderate, label: Text('Moderate')),
                  ButtonSegment(value: HikingDifficulty.hard, label: Text('Hard')),
                  ButtonSegment(value: HikingDifficulty.expert, label: Text('Expert')),
                ],
                selected: {_difficulty},
                onSelectionChanged: (s) => setState(() => _difficulty = s.first),
              ),
              const SizedBox(height: 12),
              DropdownButtonFormField<TrailSurface>(
                initialValue: _surface,
                decoration: const InputDecoration(labelText: 'Surface', border: OutlineInputBorder()),
                items: TrailSurface.values.map((s) =>
                  DropdownMenuItem(value: s, child: Text(s.name))).toList(),
                onChanged: (v) => setState(() => _surface = v ?? TrailSurface.path),
              ),
              const SizedBox(height: 12),
              DropdownButtonFormField<TrailMarking>(
                initialValue: _marking,
                decoration: const InputDecoration(labelText: 'Marking', border: OutlineInputBorder()),
                items: TrailMarking.values.map((m) =>
                  DropdownMenuItem(value: m, child: Text(m.name))).toList(),
                onChanged: (v) => setState(() => _marking = v ?? TrailMarking.signposted),
              ),
              const SizedBox(height: 8),
              SwitchListTile(value: _water, onChanged: (v) => setState(() => _water = v),
                title: const Text('Water sources along trail'), contentPadding: EdgeInsets.zero),
              SwitchListTile(value: _shelter, onChanged: (v) => setState(() => _shelter = v),
                title: const Text('Shelter or hut on route'), contentPadding: EdgeInsets.zero),
              const SizedBox(height: 24),
              FilledButton.icon(
                onPressed: _saving ? null : _save,
                icon: _saving
                  ? const SizedBox(width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2))
                  : const Icon(Icons.check),
                label: const Text('Save'),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _num(TextEditingController c, String label) => TextFormField(
    controller: c, keyboardType: TextInputType.number,
    decoration: InputDecoration(labelText: label, border: const OutlineInputBorder(), isDense: true),
    validator: (v) => double.tryParse(v ?? '') == null ? 'Number required' : null);

  Future<void> _save() async {
    if (!(_formKey.currentState?.validate() ?? false)) return;
    setState(() => _saving = true);
    try {
      final details = HikingDetails(
        distanceMeters: int.parse(_distance.text),
        ascentMeters: int.parse(_ascent.text),
        descentMeters: int.parse(_descent.text),
        elevationMinMeters: int.parse(_elevMin.text),
        elevationMaxMeters: int.parse(_elevMax.text),
        difficulty: _difficulty, surface: _surface, marking: _marking,
        estimatedHours: double.tryParse(_hours.text),
        hasWaterSources: _water, hasShelter: _shelter,
      );
      await ref.read(hikingRepositoryProvider).create(
        name: _name.text.trim(),
        description: _description.text.trim().isEmpty ? null : _description.text.trim(),
        route: _route, details: details);
      if (mounted) Navigator.of(context).pop();
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Failed to save: $e')));
      }
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }
}
