import 'package:flutter/material.dart';
import 'package:turbo/l10n/app_localizations.dart';
import '../models/named_icon.dart';
import 'icon_selection_page.dart';

class LocationFormFields extends StatelessWidget {
  final TextEditingController nameController;
  final TextEditingController descriptionController;
  final NamedIcon selectedIcon;
  final Function(NamedIcon) onIconSelected;

  const LocationFormFields({
    super.key,
    required this.nameController,
    required this.descriptionController,
    required this.selectedIcon,
    required this.onIconSelected,
  });

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        TextFormField(
          controller: nameController,
          decoration: InputDecoration(
            labelText: l10n.name,
            border: OutlineInputBorder(borderRadius: BorderRadius.circular(16)
            ),
          ),
          validator: (value) {
            if (value == null || value.isEmpty) {
              return l10n.pleaseEnterName;
            }
            return null;
          },
        ),
        const SizedBox(height: 16),
        TextFormField(
          controller: descriptionController,
          maxLines: 3,
          minLines: 1,
          decoration: InputDecoration(
            labelText: l10n.descriptionOptional,
            border: OutlineInputBorder(borderRadius: BorderRadius.circular(16)
            ),
          ),
        ),
        const SizedBox(height: 24),
        Text(
          l10n.icon,
          style: Theme.of(context).textTheme.titleSmall,
        ),
        const SizedBox(height: 8),
        IconSelector(
          selectedIcon: selectedIcon,
          onIconSelected: onIconSelected,
        ),
      ],
    );
  }
}

class IconSelector extends StatelessWidget {
  final NamedIcon selectedIcon;
  final Function(NamedIcon) onIconSelected;

  const IconSelector({
    super.key,
    required this.selectedIcon,
    required this.onIconSelected,
  });

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;

    return Material(
      color: colorScheme.surfaceContainer,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
      ),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: () async {
          final NamedIcon? result =
          await IconSelectionPage.show(context);
          if (result != null) {
            onIconSelected(result);
          }
        },
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
          child: Row(
            children: [
              Container(
                width: 40,
                height: 40,
                decoration: BoxDecoration(
                  color: colorScheme.primaryContainer,
                  shape: BoxShape.circle,
                ),
                child: Icon(
                  selectedIcon.icon,
                  color: colorScheme.onPrimaryContainer,
                  size: 24,
                ),
              ),
              const SizedBox(width: 16),
              Expanded(
                child: Text(
                  selectedIcon.localizedTitle ?? selectedIcon.title,
                  style: textTheme.bodyLarge?.copyWith(
                    color: colorScheme.onSurface,
                  ),
                ),
              ),
              Icon(
                Icons.chevron_right,
                color: colorScheme.onSurfaceVariant,
              ),
            ],
          ),
        ),
      ),
    );
  }
}