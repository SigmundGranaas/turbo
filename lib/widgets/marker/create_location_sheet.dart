import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/data/icon_service.dart';
import 'package:turbo/data/model/named_icon.dart';
import 'package:turbo/l10n/app_localizations.dart';
import '../../data/model/marker.dart';
import '../../data/state/providers/location_repository.dart';
import 'components.dart';
import '../auth/primary_button.dart';

class CreateLocationSheet extends ConsumerStatefulWidget {
  final LatLng? newLocation;

  const CreateLocationSheet({super.key, this.newLocation});

  @override
  ConsumerState<CreateLocationSheet> createState() =>
      CreateLocationSheetState();
}

class CreateLocationSheetState extends ConsumerState<CreateLocationSheet> {
  final _formKey = GlobalKey<FormState>();
  late NamedIcon _selectedIcon;
  late TextEditingController _nameController;
  late TextEditingController _descriptionController;
  bool _isLoading = false;
  bool _isInitialized = false;

  @override
  void initState() {
    super.initState();
    _nameController = TextEditingController();
    _descriptionController = TextEditingController();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (!_isInitialized) {
      _selectedIcon = IconService().getDefaultIcon(context);
      _isInitialized = true;
    }
  }

  @override
  void dispose() {
    _nameController.dispose();
    _descriptionController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final textTheme = Theme.of(context).textTheme;
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
                Text(l10n.newMarker, style: textTheme.titleLarge),
                IconButton(
                  onPressed: () => Navigator.pop(context),
                  icon: const Icon(Icons.close),
                ),
              ],
            ),
            Flexible(
              child: SingleChildScrollView(
                child: Padding(
                  padding: const EdgeInsets.symmetric(vertical: 24),
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
              text: l10n.saveMarker,
              onPressed: _isLoading ? null : _saveLocation,
              isLoading: _isLoading,
            ),
          ],
        ),
      ),
    );
  }

  void _saveLocation() async {
    final l10n = context.l10n;
    if (_formKey.currentState!.validate() && widget.newLocation != null) {
      setState(() => _isLoading = true);

      try {
        final locationNotifier = ref.read(locationRepositoryProvider.notifier);

        final newMarker = Marker(
          title: _nameController.text,
          description: _descriptionController.text.isEmpty
              ? null
              : _descriptionController.text,
          icon: _selectedIcon.title, // Use the non-translated key for saving
          position: widget.newLocation!,
        );

        await locationNotifier.addMarker(newMarker);

        if (!mounted) return;
        Navigator.of(context).pop(newMarker);
      } catch (e) {
        if (!mounted) return;
        _showErrorSnackBar(context, e.toString());
      } finally {
        if (mounted) {
          setState(() => _isLoading = false);
        }
      }
    } else if (widget.newLocation == null) {
      _showErrorSnackBar(context, l10n.errorLocationNotSpecified);
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