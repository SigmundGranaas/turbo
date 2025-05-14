import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:map_app/data/icon_service.dart';
import 'package:map_app/data/model/named_icon.dart';
import '../../data/model/marker.dart';
import '../../data/state/providers/location_repository.dart';
import 'components.dart'; // Assuming LocationFormFields is here
import '../auth/primary_button.dart'; // For PrimaryButton

class CreateLocationSheet extends ConsumerStatefulWidget {
  final LatLng? newLocation; // Marker is not passed for creation, only LatLng

  const CreateLocationSheet({super.key, this.newLocation});

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
    // Initialize with a default icon or the first from IconService
    _selectedIcon = IconService().getAllIcons().firstOrNull ?? const NamedIcon(title: 'Default', icon: Icons.help_outline);
    _nameController = TextEditingController();
    _descriptionController = TextEditingController();
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
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
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
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Text(
                    'New Marker',
                    style: textTheme.titleLarge?.copyWith(fontWeight: FontWeight.bold),
                  ),
                  IconButton(
                    onPressed: () => Navigator.pop(context),
                    icon: const Icon(Icons.close),
                    style: IconButton.styleFrom(
                      foregroundColor: colorScheme.onSurfaceVariant,
                      backgroundColor: colorScheme.surfaceContainerHighest.withOpacity(0.3),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 24),
              LocationFormFields(
                nameController: _nameController,
                descriptionController: _descriptionController,
                selectedIcon: _selectedIcon,
                onIconSelected: (icon) => setState(() => _selectedIcon = icon),
              ),
              const SizedBox(height: 32),
              SizedBox(
                width: double.infinity,
                child: PrimaryButton(
                  text: 'Save Marker',
                  onPressed: _isLoading ? null : _saveLocation,
                  isLoading: _isLoading,
                ),
              ),
              const SizedBox(height: 8),
            ],
          ),
        ),
      ),
    );
  }

  void _saveLocation() async {
    if (_formKey.currentState!.validate() && widget.newLocation != null) {
      setState(() => _isLoading = true);

      try {
        final locationNotifier = ref.read(locationRepositoryProvider.notifier);

        final newMarker = Marker(
          title: _nameController.text,
          description: _descriptionController.text.isEmpty ? null : _descriptionController.text,
          icon: _selectedIcon.title,
          position: widget.newLocation!,
          // `synced` will be handled by the repository
        );

        await locationNotifier.addMarker(newMarker);

        if (!mounted) return;
        Navigator.of(context).pop(newMarker); // Pass back the created marker
      } catch (e) {
        if (!mounted) return;
        _showErrorSnackBar(context, e.toString());
      } finally {
        if (mounted) {
          setState(() => _isLoading = false);
        }
      }
    } else if (widget.newLocation == null) {
      _showErrorSnackBar(context, "Location not specified for new marker.");
    }
  }

  void _showErrorSnackBar(BuildContext context, String message) {
    final colorScheme = Theme.of(context).colorScheme;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Row(
          children: [
            Icon(Icons.error_outline, color: colorScheme.onErrorContainer),
            const SizedBox(width: 16),
            Expanded(child: Text(message)),
          ],
        ),
        behavior: SnackBarBehavior.floating,
        backgroundColor: colorScheme.errorContainer,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
        margin: const EdgeInsets.all(16),
      ),
    );
  }
}