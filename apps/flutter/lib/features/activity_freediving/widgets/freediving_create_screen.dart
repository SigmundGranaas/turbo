import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/features/activities/api.dart';

import '../data/freediving_repository.dart';
import '../models/freediving_activity.dart';
import '../models/freediving_details.dart';

class FreedivingCreateScreen extends ConsumerStatefulWidget {
  final ActivityGeometry seedGeometry;

  /// If non-null the form opens in edit mode: fields are prefilled
  /// from this activity and Save calls `update` instead of `create`.
  final FreedivingActivity? existing;

  const FreedivingCreateScreen({
    super.key,
    required this.seedGeometry,
    this.existing,
  });

  @override
  ConsumerState<FreedivingCreateScreen> createState() => _FreedivingCreateScreenState();
}

class _FreedivingCreateScreenState extends ConsumerState<FreedivingCreateScreen> {
  final _formKey = GlobalKey<FormState>();
  final _name = TextEditingController();
  final _description = TextEditingController();
  final _depth = TextEditingController(text: '12');
  final _visibility = TextEditingController(text: '5');
  final _accessNotes = TextEditingController();
  WaterBody _waterBody = WaterBody.sea;
  BottomType _bottom = BottomType.kelpForest;
  bool _harpoon = false;
  bool _shoreEntry = true;
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    final e = widget.existing;
    if (e != null) {
      _name.text = e.name;
      _description.text = e.description ?? '';
      _depth.text = e.details.maxDepthMeters.toString();
      _visibility.text = e.details.typicalVisibilityMeters?.toString() ?? '';
      _accessNotes.text = e.details.accessNotes ?? '';
      _waterBody = e.details.waterBody;
      _bottom = e.details.bottomType;
      _harpoon = e.details.harpoonAllowed;
      _shoreEntry = e.details.shoreEntry;
    }
  }

  @override
  void dispose() {
    _name.dispose(); _description.dispose();
    _depth.dispose(); _visibility.dispose(); _accessNotes.dispose();
    super.dispose();
  }

  LatLng get _position =>
      widget.existing?.position ??
      widget.seedGeometry.firstPoint ??
      const LatLng(0, 0);

  bool get _isEdit => widget.existing != null;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text(_isEdit ? 'Edit freediving spot' : 'New freediving spot')),
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
          Text('Water body', style: Theme.of(context).textTheme.labelLarge),
          const SizedBox(height: 4),
          SegmentedButton<WaterBody>(
            segments: const [
              ButtonSegment(value: WaterBody.sea, label: Text('Sea')),
              ButtonSegment(value: WaterBody.fjord, label: Text('Fjord')),
              ButtonSegment(value: WaterBody.lake, label: Text('Lake')),
            ],
            selected: {_waterBody},
            onSelectionChanged: (s) => setState(() => _waterBody = s.first)),
          const SizedBox(height: 12),
          DropdownButtonFormField<BottomType>(
            initialValue: _bottom,
            decoration: const InputDecoration(labelText: 'Bottom', border: OutlineInputBorder()),
            items: BottomType.values.map((b) =>
              DropdownMenuItem(value: b, child: Text(b.name))).toList(),
            onChanged: (v) => setState(() => _bottom = v ?? BottomType.kelpForest)),
          const SizedBox(height: 12),
          Row(children: [
            Expanded(child: _num(_depth, 'Max depth m', required: true)),
            const SizedBox(width: 8),
            Expanded(child: _num(_visibility, 'Visibility m', required: false)),
          ]),
          const SizedBox(height: 8),
          SwitchListTile(value: _shoreEntry, onChanged: (v) => setState(() => _shoreEntry = v),
            title: const Text('Shore entry'), contentPadding: EdgeInsets.zero),
          SwitchListTile(value: _harpoon, onChanged: (v) => setState(() => _harpoon = v),
            title: const Text('Harpoon fishing allowed'), contentPadding: EdgeInsets.zero),
          const SizedBox(height: 8),
          TextFormField(controller: _accessNotes, maxLines: 2,
            decoration: const InputDecoration(labelText: 'Access notes (optional)', border: OutlineInputBorder())),
          const SizedBox(height: 12),
          ListTile(
            leading: const Icon(Icons.location_on_outlined),
            title: const Text('Position'),
            subtitle: Text('${_position.latitude.toStringAsFixed(5)}, ${_position.longitude.toStringAsFixed(5)}'),
            contentPadding: EdgeInsets.zero),
          const SizedBox(height: 16),
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

  Future<void> _save() async {
    if (!(_formKey.currentState?.validate() ?? false)) return;
    setState(() => _saving = true);
    try {
      final details = FreedivingDetails(
        waterBody: _waterBody, bottomType: _bottom,
        maxDepthMeters: double.parse(_depth.text),
        typicalVisibilityMeters: double.tryParse(_visibility.text),
        harpoonAllowed: _harpoon, shoreEntry: _shoreEntry,
        accessNotes: _accessNotes.text.trim().isEmpty ? null : _accessNotes.text.trim());
      final repo = ref.read(freedivingRepositoryProvider);
      final existing = widget.existing;
      if (existing != null) {
        await repo.update(
          id: existing.id,
          name: _name.text.trim(),
          description: _description.text.trim().isEmpty ? null : _description.text.trim(),
          details: details);
      } else {
        await repo.create(
          name: _name.text.trim(),
          description: _description.text.trim().isEmpty ? null : _description.text.trim(),
          position: _position, details: details);
      }
      if (mounted) Navigator.of(context).pop();
    } catch (e) {
      if (mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Failed: $e')));
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }
}
