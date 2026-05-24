import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import 'package:turbo/features/activities/api.dart';

import '../data/fishing_repository.dart';
import '../models/fishing_activity.dart';
import '../models/fishing_details.dart';

/// Create or edit form for a fishing activity. Composed of small typed
/// widgets; every field maps to a typed property on [FishingDetails] —
/// no catch-all map. Geometry is seeded from the picker's tap point on
/// create; on edit the existing activity's position is used and the
/// form prefills from its current values.
class FishingCreateScreen extends ConsumerStatefulWidget {
  final ActivityGeometry seedGeometry;

  /// If non-null the form opens in edit mode: fields are prefilled
  /// from this activity, the AppBar title changes to "Edit fishing
  /// spot", and Save calls `update` instead of `create`.
  final FishingActivity? existing;

  const FishingCreateScreen({
    super.key,
    required this.seedGeometry,
    this.existing,
  });

  @override
  ConsumerState<FishingCreateScreen> createState() => _FishingCreateScreenState();
}

class _FishingCreateScreenState extends ConsumerState<FishingCreateScreen> {
  final _formKey = GlobalKey<FormState>();
  final _name = TextEditingController();
  final _description = TextEditingController();
  final _accessNotes = TextEditingController();
  WaterKind _waterKind = WaterKind.river;
  ShoreOrBoat _shoreOrBoat = ShoreOrBoat.shore;
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    final e = widget.existing;
    if (e != null) {
      _name.text = e.name;
      _description.text = e.description ?? '';
      _accessNotes.text = e.details.accessNotes ?? '';
      _waterKind = e.details.waterKind;
      _shoreOrBoat = e.details.shoreOrBoat;
    }
  }

  @override
  void dispose() {
    _name.dispose();
    _description.dispose();
    _accessNotes.dispose();
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
      appBar: AppBar(
        title: Text(_isEdit ? 'Edit fishing spot' : 'New fishing spot'),
      ),
      body: SafeArea(
        child: Form(
          key: _formKey,
          child: ListView(
            padding: const EdgeInsets.all(16),
            children: [
              TextFormField(
                controller: _name,
                decoration: const InputDecoration(
                  labelText: 'Name',
                  border: OutlineInputBorder(),
                ),
                validator: (v) => (v == null || v.trim().isEmpty) ? 'Required' : null,
              ),
              const SizedBox(height: 12),
              TextFormField(
                controller: _description,
                decoration: const InputDecoration(
                  labelText: 'Description (optional)',
                  border: OutlineInputBorder(),
                ),
                maxLines: 2,
              ),
              const SizedBox(height: 16),
              Text('Water kind', style: Theme.of(context).textTheme.labelLarge),
              const SizedBox(height: 4),
              SegmentedButton<WaterKind>(
                segments: const [
                  ButtonSegment(value: WaterKind.river, label: Text('River')),
                  ButtonSegment(value: WaterKind.lake, label: Text('Lake')),
                  ButtonSegment(value: WaterKind.sea, label: Text('Sea')),
                ],
                selected: {_waterKind},
                onSelectionChanged: (s) => setState(() => _waterKind = s.first),
              ),
              const SizedBox(height: 16),
              Text('Access', style: Theme.of(context).textTheme.labelLarge),
              const SizedBox(height: 4),
              SegmentedButton<ShoreOrBoat>(
                segments: const [
                  ButtonSegment(value: ShoreOrBoat.shore, label: Text('Shore')),
                  ButtonSegment(value: ShoreOrBoat.boat, label: Text('Boat')),
                  ButtonSegment(value: ShoreOrBoat.either, label: Text('Either')),
                ],
                selected: {_shoreOrBoat},
                onSelectionChanged: (s) => setState(() => _shoreOrBoat = s.first),
              ),
              const SizedBox(height: 12),
              TextFormField(
                controller: _accessNotes,
                decoration: const InputDecoration(
                  labelText: 'Access notes (optional)',
                  border: OutlineInputBorder(),
                  hintText: 'Where to park, scrambles, gates, …',
                ),
                maxLines: 2,
              ),
              const SizedBox(height: 12),
              ListTile(
                leading: const Icon(Icons.location_on_outlined),
                title: const Text('Location'),
                subtitle: Text(
                  '${_position.latitude.toStringAsFixed(5)}, '
                  '${_position.longitude.toStringAsFixed(5)}',
                ),
                contentPadding: EdgeInsets.zero,
              ),
              const SizedBox(height: 24),
              FilledButton.icon(
                onPressed: _saving ? null : _save,
                icon: _saving
                    ? const SizedBox(
                        width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2))
                    : const Icon(Icons.check),
                label: const Text('Save'),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Future<void> _save() async {
    if (!(_formKey.currentState?.validate() ?? false)) return;
    setState(() => _saving = true);
    try {
      final details = FishingDetails(
        waterKind: _waterKind,
        shoreOrBoat: _shoreOrBoat,
        accessNotes: _accessNotes.text.trim().isEmpty ? null : _accessNotes.text.trim(),
      );
      final repo = ref.read(fishingRepositoryProvider);
      final existing = widget.existing;
      if (existing != null) {
        await repo.update(
          id: existing.id,
          name: _name.text.trim(),
          description:
              _description.text.trim().isEmpty ? null : _description.text.trim(),
          details: details,
        );
      } else {
        await repo.create(
          name: _name.text.trim(),
          description:
              _description.text.trim().isEmpty ? null : _description.text.trim(),
          position: _position,
          details: details,
        );
      }
      // Pop with `true` so parent surfaces (path detail sheet,
      // marker detail sheet, picker) know to auto-dismiss instead of
      // leaving the user buried under modals after a successful save.
      if (mounted) Navigator.of(context).pop(true);
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Failed to save: $e')),
        );
      }
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }
}
