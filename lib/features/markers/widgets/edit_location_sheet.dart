import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/l10n/app_localizations.dart';
import '../data/icon_service.dart';
import '../models/marker.dart';
import '../models/named_icon.dart';
import '../data/location_repository.dart';
import 'components.dart';
import 'marker_info_sheet.dart';
import 'package:turbo/core/widgets/buttons/primary_button.dart';

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
  NamedIcon? _selectedIcon;
  bool _isLoading = false;
  bool _isInitialized = false;

  @override
  void initState() {
    super.initState();
    _nameController = TextEditingController(text: widget.location.title);
    _descriptionController =
        TextEditingController(text: widget.location.description);
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (!_isInitialized) {
      _selectedIcon = widget.location.icon != null
          ? IconService().getIcon(context, widget.location.icon)
          : null;
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
    final mediaQuery = MediaQuery.of(context);
    final bottomPadding = mediaQuery.viewInsets.bottom > 0
        ? mediaQuery.viewInsets.bottom
        : mediaQuery.padding.bottom;

    return Padding(
      padding: EdgeInsets.fromLTRB(16, 16, 16, 16 + bottomPadding),
      child: Form(
        key: _formKey,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Text(l10n.editMarker, style: textTheme.titleLarge),
                IconButton(
                  onPressed: () => Navigator.pop(context),
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
                    onIconSelected: (icon) => setState(() => _selectedIcon = icon),
                  ),
                ),
              ),
            ),
            PrimaryButton(
              text: l10n.saveChanges,
              onPressed: _isLoading ? null : _updateLocation,
              isLoading: _isLoading,
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _updateLocation() async {
    final l10n = context.l10n;
    if (_formKey.currentState!.validate()) {
      setState(() => _isLoading = true);
      try {
        final updatedMarker = widget.location.copyWith(
          title: _nameController.text,
          description: _descriptionController.text.isEmpty
              ? null
              : _descriptionController.text,
          icon: _selectedIcon?.title,
        );

        await ref
            .read(locationRepositoryProvider.notifier)
            .updateMarker(updatedMarker);

        if (mounted) {
          Navigator.of(context).pop(MarkerInfoResult.updated);
        }
      } catch (error) {
        if (mounted) {
          _showErrorSnackBar(context, l10n.errorUpdatingLocation(error.toString()));
        }
      } finally {
        if (mounted) {
          setState(() => _isLoading = false);
        }
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