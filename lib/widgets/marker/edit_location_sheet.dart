import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../data/icon_service.dart';
import '../../data/model/marker.dart';
import '../../data/model/named_icon.dart';
import '../../data/state/providers/location_repository.dart';
import 'components.dart'; // Assuming LocationFormFields is here
import '../auth/primary_button.dart'; // For PrimaryButton

class EditLocationSheet extends ConsumerStatefulWidget {
  final Marker location;

  const EditLocationSheet({super.key, required this.location});

  @override
  ConsumerState<EditLocationSheet> createState() => EditLocationSheetState();
}

class EditLocationSheetState extends ConsumerState<EditLocationSheet> {
  final _formKey = GlobalKey<FormState>();
  late final TextEditingController _nameController;
  late final TextEditingController _descriptionController;
  late NamedIcon _selectedIcon;
  bool _isLoading = false;
  bool _isDeleting = false;

  @override
  void initState() {
    super.initState();
    _nameController = TextEditingController(text: widget.location.title);
    _descriptionController = TextEditingController(text: widget.location.description);
    _selectedIcon = IconService().getIcon(widget.location.icon);
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
                    'Edit Marker',
                    style: textTheme.titleLarge?.copyWith(fontWeight: FontWeight.bold),
                  ),
                  IconButton(
                    onPressed: () => Navigator.pop(context, false), // Return false if no change
                    icon: const Icon(Icons.close),
                    style: IconButton.styleFrom(
                      foregroundColor: colorScheme.onSurfaceVariant,
                      backgroundColor: colorScheme.surfaceContainerHighest.withValues(alpha: 0.3),
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
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    PrimaryButton(
                      text: 'Save Changes',
                      onPressed: _isLoading ? null : _updateLocation,
                      isLoading: _isLoading,
                    ),
                    const SizedBox(height: 16),
                    OutlinedButton.icon(
                      icon: _isDeleting
                          ? SizedBox(
                        width: 16, height: 16,
                        child: CircularProgressIndicator(strokeWidth: 2, valueColor: AlwaysStoppedAnimation<Color>(colorScheme.error)),
                      )
                          : Icon(Icons.delete_outline, color: colorScheme.error),
                      label: Text('Delete Marker', style: textTheme.labelLarge?.copyWith(color: colorScheme.error, fontWeight: FontWeight.w500)),
                      onPressed: _isDeleting || _isLoading ? null : _confirmDelete,
                      style: OutlinedButton.styleFrom(
                        foregroundColor: colorScheme.error,
                        side: BorderSide(color: colorScheme.error),
                        padding: const EdgeInsets.symmetric(vertical: 18), // Increased padding
                        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(28)), // More rounded
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  void _confirmDelete() {
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    showDialog(
      context: context,
      builder: (BuildContext dialogContext) {
        return AlertDialog(
          title: Text('Delete Marker', style: textTheme.titleLarge?.copyWith(fontWeight: FontWeight.bold)),
          content: Text('Are you sure you want to delete this marker? This action cannot be undone.', style: textTheme.bodyMedium),
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
          icon: Icon(Icons.warning_amber_rounded, color: colorScheme.error, size: 28),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(),
              child: Text('Cancel', style: textTheme.labelLarge?.copyWith(color: colorScheme.primary)),
            ),
            ElevatedButton(
              onPressed: () {
                Navigator.of(dialogContext).pop();
                _deleteLocation();
              },
              style: ElevatedButton.styleFrom(
                backgroundColor: colorScheme.errorContainer,
                foregroundColor: colorScheme.onErrorContainer,
                elevation: 0,
                padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
                shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
              ),
              child: Text('Delete', style: textTheme.labelLarge?.copyWith(color: colorScheme.onErrorContainer, fontWeight: FontWeight.w500)),
            ),
          ],
        );
      },
    );
  }

  Future<void> _updateLocation() async {
    if (_formKey.currentState!.validate()) {
      setState(() => _isLoading = true);
      try {
        final updatedMarker = widget.location.copyWith(
          title: _nameController.text,
          description: _descriptionController.text.isEmpty ? null : _descriptionController.text,
          icon: _selectedIcon.title,
        );

        await ref.read(locationRepositoryProvider.notifier).updateMarker(updatedMarker);

        if (mounted) {
          Navigator.of(context).pop(true); // Return true indicating changes were made
        }
      } catch (error) {
        if (mounted) {
          _showErrorSnackBar(context, 'Error updating location: $error');
        }
      } finally {
        if (mounted) {
          setState(() => _isLoading = false);
        }
      }
    }
  }

  Future<void> _deleteLocation() async {
    setState(() => _isDeleting = true);
    try {
      await ref.read(locationRepositoryProvider.notifier).deleteMarker(widget.location.uuid);
      if (mounted) {
        Navigator.of(context).pop(true); // Return true indicating changes were made
      }
    } catch (error) {
      if (mounted) {
        _showErrorSnackBar(context, 'Error deleting location: $error');
      }
    } finally {
      if (mounted) {
        setState(() => _isDeleting = false);
      }
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