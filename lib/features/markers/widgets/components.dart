import 'package:flutter/material.dart';
import 'package:turbo/l10n/app_localizations.dart';
import '../models/named_icon.dart';
import 'icon_selection_page.dart';

class LocationFormFields extends StatefulWidget {
  final TextEditingController nameController;
  final TextEditingController descriptionController;
  final NamedIcon? selectedIcon;
  final Function(NamedIcon?) onIconSelected;

  const LocationFormFields({
    super.key,
    required this.nameController,
    required this.descriptionController,
    required this.selectedIcon,
    required this.onIconSelected,
  });

  @override
  State<LocationFormFields> createState() => _LocationFormFieldsState();
}

class _LocationFormFieldsState extends State<LocationFormFields> {
  late bool _showDescription;

  @override
  void initState() {
    super.initState();
    _showDescription = widget.descriptionController.text.isNotEmpty;
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        TextFormField(
          controller: widget.nameController,
          decoration: InputDecoration(
            labelText: l10n.name,
            border: OutlineInputBorder(borderRadius: BorderRadius.circular(16)),
          ),
          validator: (value) {
            if (value == null || value.trim().isEmpty) {
              return l10n.pleaseEnterName;
            }
            return null;
          },
        ),
        const SizedBox(height: 16),
        if (_showDescription)
          TextFormField(
            controller: widget.descriptionController,
            maxLines: 2,
            decoration: InputDecoration(
              labelText: l10n.descriptionOptional,
              border: OutlineInputBorder(borderRadius: BorderRadius.circular(16)),
            ),
          )
        else
          Align(
            alignment: Alignment.centerLeft,
            child: TextButton.icon(
              onPressed: () => setState(() => _showDescription = true),
              icon: const Icon(Icons.add, size: 18),
              label: Text(l10n.addDescription),
            ),
          ),
        const SizedBox(height: 24),
        Text(
          l10n.icon,
          style: Theme.of(context).textTheme.titleSmall,
        ),
        const SizedBox(height: 8),
        IconSelector(
          selectedIcon: widget.selectedIcon,
          onIconSelected: widget.onIconSelected,
        ),
      ],
    );
  }
}

class IconSelector extends StatelessWidget {
  final NamedIcon? selectedIcon;
  final Function(NamedIcon?) onIconSelected;

  const IconSelector({
    super.key,
    required this.selectedIcon,
    required this.onIconSelected,
  });

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final textTheme = Theme.of(context).textTheme;
    final l10n = context.l10n;
    final hasIcon = selectedIcon != null;

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
          FocusManager.instance.primaryFocus?.unfocus();
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
                  color: hasIcon
                      ? colorScheme.primaryContainer
                      : colorScheme.surfaceContainerHighest,
                  shape: BoxShape.circle,
                ),
                child: Icon(
                  hasIcon ? selectedIcon!.icon : Icons.add,
                  color: hasIcon
                      ? colorScheme.onPrimaryContainer
                      : colorScheme.onSurfaceVariant,
                  size: 24,
                ),
              ),
              const SizedBox(width: 16),
              Expanded(
                child: Text(
                  hasIcon
                      ? (selectedIcon!.localizedTitle ?? selectedIcon!.title)
                      : l10n.icon,
                  style: textTheme.bodyLarge?.copyWith(
                    color: colorScheme.onSurface,
                  ),
                ),
              ),
              if (hasIcon)
                IconButton(
                  icon: const Icon(Icons.clear),
                  onPressed: () => onIconSelected(null),
                )
              else
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
