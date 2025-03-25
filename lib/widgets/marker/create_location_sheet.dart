import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:idb_shim/idb.dart';
import 'package:map_app/data/model/named_icon.dart';
import 'package:latlong2/latlong.dart';
import '../../data/model/marker.dart';
import '../../data/state/providers/location_provider.dart';
import 'location_base_sheet.dart';

// Import the shared button components
import '../auth/primary_button.dart';

class CreateLocationSheet extends ConsumerStatefulWidget {
  final Marker? location;
  final LatLng? newLocation;

  const CreateLocationSheet({super.key, this.location, this.newLocation});

  @override
  ConsumerState<CreateLocationSheet> createState() => CreateLocationSheetState();
}

class CreateLocationSheetState extends ConsumerState<CreateLocationSheet> {
  final _formKey = GlobalKey<FormState>();
  late NamedIcon _selectedIcon;
  late TextEditingController _nameController;
  late TextEditingController _descriptionController;
  bool _isLoading = false;

  @override
  void initState() {
    super.initState();
    _selectedIcon = const NamedIcon(title: 'Fjell', icon: Icons.landscape);
    _nameController = TextEditingController(text: widget.location?.title ?? '');
    _descriptionController = TextEditingController(text: widget.location?.description ?? '');
  }

  @override
  void dispose() {
    _nameController.dispose();
    _descriptionController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    return Container(
      decoration: BoxDecoration(
        color: colorScheme.surface,
        borderRadius: const BorderRadius.vertical(top: Radius.circular(28)),
      ),
      padding: EdgeInsets.only(
        bottom: MediaQuery.of(context).viewInsets.bottom + 24,
        left: 24,
        right: 24,
        top: 12,
      ),
      child: Form(
        key: _formKey,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            // Drag handle
            Center(
              child: Container(
                width: 32,
                height: 4,
                decoration: BoxDecoration(
                  color: colorScheme.outlineVariant,
                  borderRadius: BorderRadius.circular(2),
                ),
              ),
            ),
            const SizedBox(height: 16),
            // Header
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Text(
                  'New Marker',
                  style: textTheme.titleLarge?.copyWith(
                    fontWeight: FontWeight.bold,
                  ),
                ),
                IconButton(
                  onPressed: () => Navigator.pop(context),
                  icon: const Icon(Icons.close),
                  style: IconButton.styleFrom(
                    foregroundColor: colorScheme.onSurfaceVariant,
                    backgroundColor: colorScheme.surfaceContainerHighest.withValues(alpha: 0.3),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 24),
            // Form fields
            LocationFormFields(
              nameController: _nameController,
              descriptionController: _descriptionController,
              selectedIcon: _selectedIcon,
              onIconSelected: (icon) => setState(() => _selectedIcon = icon),
            ),
            const SizedBox(height: 32),
            // Save button
            SizedBox(
              width: double.infinity,
              child: PrimaryButton(
                text: 'Save Marker',
                onPressed: _saveLocation,
                isLoading: _isLoading,
              ),
            ),
            const SizedBox(height: 8),
          ],
        ),
      ),
    );
  }

  void _saveLocation() async {
    if (_formKey.currentState!.validate()) {
      setState(() => _isLoading = true);

      try {
        final locationProvider = ref.read(locationNotifierProvider.notifier);

        final newMarker = Marker.fromMap({
          'title': _nameController.text,
          'description': _descriptionController.text,
          'icon': _selectedIcon.title,
          'latitude': widget.location?.position.latitude ?? widget.newLocation!.latitude,
          'longitude': widget.location?.position.longitude ?? widget.newLocation!.longitude,
        });

        await locationProvider.addLocation(newMarker);

        if (!mounted) return;
        Navigator.of(context).pop(newMarker);
      } catch (e) {
        if (!mounted) return;
        Navigator.of(context).pop();
        _showErrorSnackBar(context, e);
      } finally {
        if (mounted) {
          setState(() => _isLoading = false);
        }
      }
    }
  }

  void _showErrorSnackBar(BuildContext context, dynamic error) {
    final colorScheme = Theme.of(context).colorScheme;

    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Row(
          children: [
            Icon(Icons.error_outline, color: colorScheme.onErrorContainer),
            const SizedBox(width: 16),
            Expanded(
              child: Text(_getErrorMessage(error)),
            ),
          ],
        ),
        behavior: SnackBarBehavior.floating,
        backgroundColor: colorScheme.errorContainer,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
        margin: const EdgeInsets.all(16),
      ),
    );
  }

  String _getErrorMessage(dynamic error) {
    if (error is DatabaseException) {
      return 'Database error. Please try again later.';
    } else {
      return 'An unexpected error occurred. Please try again.';
    }
  }
}