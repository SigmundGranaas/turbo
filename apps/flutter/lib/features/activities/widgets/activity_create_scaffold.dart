import 'package:flutter/material.dart';

/// Shared chrome for every activity kind's create/edit screen.
///
/// The 6 kind create screens were near-identical around the edges (app bar
/// title, name + description fields, an optional "draw route" button, a Save
/// button with a loading spinner, the number-field helper) while differing
/// only in the middle [fields]. This widget owns the shared parts; each kind
/// composes it with its own [fields] + save logic. Composition over
/// inheritance — there is no `ActivityCreateScreen` base class; the kind
/// keeps its controllers and repository calls. See
/// `docs/architecture/2026-06-composition-overhaul-plan.md` (Phase 6).
class ActivityCreateScaffold extends StatelessWidget {
  /// App-bar title, e.g. "New hike" / "Edit hike".
  final String title;
  final GlobalKey<FormState> formKey;
  final TextEditingController nameController;
  final TextEditingController descriptionController;

  /// Kind-specific form fields, rendered between the description field and the
  /// route button / save button.
  final List<Widget> fields;

  /// Optional "draw route on map" button (route-shaped kinds supply one; point
  /// kinds leave it null).
  final Widget? routeButton;

  final bool saving;
  final Future<void> Function() onSave;

  const ActivityCreateScaffold({
    super.key,
    required this.title,
    required this.formKey,
    required this.nameController,
    required this.descriptionController,
    required this.fields,
    required this.saving,
    required this.onSave,
    this.routeButton,
  });

  /// Shared numeric field used by every kind's stats inputs.
  static Widget numberField(
    TextEditingController controller,
    String label, {
    bool required = true,
  }) {
    return TextFormField(
      controller: controller,
      keyboardType: TextInputType.number,
      decoration: InputDecoration(
        labelText: label,
        border: const OutlineInputBorder(),
        isDense: true,
      ),
      validator: (v) {
        final s = v?.trim() ?? '';
        if (s.isEmpty) return required ? 'Number required' : null;
        return double.tryParse(s) == null ? 'Number required' : null;
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text(title)),
      body: SafeArea(
        child: Form(
          key: formKey,
          child: ListView(
            padding: const EdgeInsets.all(16),
            children: [
              TextFormField(
                controller: nameController,
                decoration: const InputDecoration(
                    labelText: 'Name', border: OutlineInputBorder()),
                validator: (v) =>
                    (v?.trim().isEmpty ?? true) ? 'Required' : null,
              ),
              const SizedBox(height: 12),
              TextFormField(
                controller: descriptionController,
                maxLines: 2,
                decoration: const InputDecoration(
                    labelText: 'Description (optional)',
                    border: OutlineInputBorder()),
              ),
              const SizedBox(height: 16),
              ...fields,
              if (routeButton != null) ...[
                const SizedBox(height: 16),
                routeButton!,
              ],
              const SizedBox(height: 24),
              FilledButton.icon(
                onPressed: saving ? null : onSave,
                icon: saving
                    ? const SizedBox(
                        width: 16,
                        height: 16,
                        child: CircularProgressIndicator(strokeWidth: 2))
                    : const Icon(Icons.check),
                label: const Text('Save'),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
