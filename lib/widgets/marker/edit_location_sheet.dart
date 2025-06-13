import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../data/icon_service.dart';
import '../../data/model/marker.dart';
import '../../data/model/named_icon.dart';
import '../../data/state/providers/location_repository.dart';
import 'components.dart';
import '../auth/primary_button.dart';

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
    _descriptionController =
        TextEditingController(text: widget.location.description);
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
    final textTheme = Theme.of(context).textTheme;
    final colorScheme = Theme.of(context).colorScheme;
    final viewInsets = MediaQuery.of(context).viewInsets;

    return Padding(
      padding: EdgeInsets.fromLTRB(16, 16, 16, 16 + viewInsets.bottom),
      child: Form(
        key: _formKey,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Text('Edit Marker', style: textTheme.titleLarge),
                IconButton(
                  onPressed: () => Navigator.pop(context, false),
                  icon: const Icon(Icons.close),
                ),
              ],
            ),
            Flexible(
              child: SingleChildScrollView(
                child: Padding(
                  padding: const EdgeInsets.symmetric(vertical: 24.0),
                  child: LocationFormFields(
                    nameController: _nameController,
                    descriptionController: _descriptionController,
                    selectedIcon: _selectedIcon,
                    onIconSelected: (icon) =>
                        setState(() => _selectedIcon = icon),
                  ),
                ),
              ),
            ),
            PrimaryButton(
              text: 'Save Changes',
              onPressed: _isLoading || _isDeleting ? null : _updateLocation,
              isLoading: _isLoading,
            ),
            const SizedBox(height: 12),
            OutlinedButton.icon(
              icon: _isDeleting
                  ? SizedBox.square(
                  dimension: 18,
                  child: CircularProgressIndicator(
                      strokeWidth: 2, color: colorScheme.error))
                  : const Icon(Icons.delete_outline),
              label: const Text('Delete Marker'),
              onPressed: _isLoading || _isDeleting ? null : _confirmDelete,
              style: OutlinedButton.styleFrom(
                foregroundColor: colorScheme.error,
                side: BorderSide(color: colorScheme.error.withValues(alpha: 0.5)),
                padding: const EdgeInsets.symmetric(vertical: 12),
                shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(100)),
              ),
            ),
          ],
        ),
      ),
    );
  }

  void _confirmDelete() {
    showDialog(
      context: context,
      builder: (BuildContext dialogContext) {
        return AlertDialog.adaptive(
          title: const Text('Delete Marker?'),
          content:
          const Text('This action is permanent and cannot be undone.'),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(),
              child: const Text('Cancel'),
            ),
            FilledButton(
              style: FilledButton.styleFrom(
                backgroundColor: Theme.of(context).colorScheme.error,
                foregroundColor: Theme.of(context).colorScheme.onError,
              ),
              onPressed: () {
                Navigator.of(dialogContext).pop();
                _deleteLocation();
              },
              child: const Text('Delete'),
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
          description: _descriptionController.text.isEmpty
              ? null
              : _descriptionController.text,
          icon: _selectedIcon.title,
        );

        await ref
            .read(locationRepositoryProvider.notifier)
            .updateMarker(updatedMarker);

        if (mounted) {
          Navigator.of(context).pop(true);
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
      await ref
          .read(locationRepositoryProvider.notifier)
          .deleteMarker(widget.location.uuid);
      if (mounted) {
        Navigator.of(context).pop(true);
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
        content: Text(message),
        behavior: SnackBarBehavior.floating,
        backgroundColor: colorScheme.errorContainer,
        shape:
        RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
        margin: const EdgeInsets.all(16),
      ),
    );
  }
}