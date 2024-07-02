import 'package:flutter/material.dart';
import '../../data/icon_service.dart';
import '../../data/model/named_icon.dart';
import '../pages/icon_selection_page.dart';

class LocationSheetBase extends StatefulWidget {
  final String title;
  final String initialName;
  final String? initialDescription;
  final NamedIcon initialIcon;
  final Widget Function(BuildContext, String, String, NamedIcon) buildButtons;

  const LocationSheetBase({
    super.key,
    required this.title,
    required this.initialName,
    required this.initialDescription,
    required this.initialIcon,
    required this.buildButtons,
  });

  @override
  State<LocationSheetBase> createState() => _LocationSheetBaseState();
}

class _LocationSheetBaseState extends State<LocationSheetBase> {
  final _formKey = GlobalKey<FormState>();
  late NamedIcon _selectedIcon;
  late TextEditingController _nameController;
  late TextEditingController _descriptionController;

  @override
  void initState() {
    super.initState();
    _selectedIcon = widget.initialIcon;
    _nameController = TextEditingController(text: widget.initialName);
    _descriptionController = TextEditingController(text: widget.initialDescription);
  }

  @override
  void dispose() {
    _nameController.dispose();
    _descriptionController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: EdgeInsets.only(
        bottom: MediaQuery.of(context).viewInsets.bottom,
        left: 16,
        right: 16,
        top: 16,
      ),
      child: Form(
        key: _formKey,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            _buildHeader(),
            _buildNameField(),
            _buildDescriptionField(),
            const SizedBox(height: 16),
            _buildIconSection(),
            const SizedBox(height: 32),
            widget.buildButtons(context, _nameController.text, _descriptionController.text, _selectedIcon),
            const SizedBox(height: 16),
          ],
        ),
      ),
    );
  }

  Widget _buildHeader() {
    return Padding(
      padding: const EdgeInsets.only(bottom: 16.0),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Text(
            widget.title,
            style: TextStyle(fontWeight: FontWeight.bold, color: Colors.grey[600]),
          ),
          IconButton(
            onPressed: () => Navigator.pop(context),
            icon: const Icon(Icons.close),
          )
        ],
      ),
    );
  }

  Widget _buildNameField() {
    return Padding(
      padding: const EdgeInsets.only(bottom: 16.0),
      child: TextFormField(
        controller: _nameController,
        decoration: const InputDecoration(
          border: OutlineInputBorder(),
          labelText: 'Navn',
        ),
        validator: (value) {
          if (value == null || value.isEmpty) {
            return 'Skriv inn et navn';
          }
          return null;
        },
      ),
    );
  }

  Widget _buildDescriptionField() {
    return Padding(
      padding: const EdgeInsets.only(bottom: 16.0),
      child: TextFormField(
        controller: _descriptionController,
        decoration: const InputDecoration(
          border: OutlineInputBorder(),
          labelText: 'Beskrivelse',
        ),
      ),
    );
  }

  Widget _buildIconSection() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          'Ikon',
          style: TextStyle(fontWeight: FontWeight.bold, color: Colors.grey[600]),
        ),
        const SizedBox(height: 4),
        _buildIconSelector(),
      ],
    );
  }

  Widget _buildIconSelector() {
    return ListTile(
      leading: Icon(_selectedIcon.icon),
      title: Text(_selectedIcon.title),
      tileColor: Colors.blue.withOpacity(0.1),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(8),
      ),
      trailing: const Icon(Icons.arrow_forward_ios),
      onTap: _selectIcon,
    );
  }

  Future<void> _selectIcon() async {
    final NamedIcon? result = await IconSelectionPage.show(context, IconService());
    if (result != null) {
      setState(() {
        _selectedIcon = result;
      });
    }
  }
}

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
    return Column(
      children: [
        TextFormField(
          controller: nameController,
          decoration: const InputDecoration(
            labelText: 'Navn',
            border: OutlineInputBorder(),
          ),
          validator: (value) {
            if (value == null || value.isEmpty) {
              return 'Skriv inn et navn';
            }
            return null;
          },
        ),
        const SizedBox(height: 16),
        TextFormField(
          controller: descriptionController,
          decoration: const InputDecoration(
            labelText: 'Beskrivelse',
            border: OutlineInputBorder(),
          ),
        ),
        const SizedBox(height: 16),
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
    Key? key,
    required this.selectedIcon,
    required this.onIconSelected,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return ListTile(
      leading: Icon(selectedIcon.icon),
      title: Text(selectedIcon.title),
      tileColor: Colors.blue.withOpacity(0.1),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(8),
      ),
      trailing: const Icon(Icons.arrow_forward_ios),
      onTap: () async {
        final NamedIcon? result = await IconSelectionPage.show(context, IconService());
        if (result != null) {
          onIconSelected(result);
        }
      },
    );
  }
}

