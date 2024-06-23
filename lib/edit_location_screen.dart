import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'location_provider.dart';

class EditLocationScreen extends StatefulWidget {
  final Map<String, dynamic> location;

  const EditLocationScreen({super.key, required this.location});

  @override
  State<EditLocationScreen> createState() => _EditLocationScreenState();
}

class _EditLocationScreenState extends State<EditLocationScreen> {
  final _formKey = GlobalKey<FormState>();
  late TextEditingController _nameController;
  late TextEditingController _descriptionController;

  @override
  void initState() {
    super.initState();
    _nameController = TextEditingController(text: widget.location['name']);
    _descriptionController = TextEditingController(text: widget.location['description']);
  }

  @override
  void dispose() {
    _nameController.dispose();
    _descriptionController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Edit Location')),
      body: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Form(
          key: _formKey,
          child: Column(
            children: [
              TextFormField(
                controller: _nameController,
                decoration: const InputDecoration(labelText: 'Name'),
                validator: (value) {
                  if (value == null || value.isEmpty) {
                    return 'Please enter a name';
                  }
                  return null;
                },
              ),
              TextFormField(
                controller: _descriptionController,
                decoration: const InputDecoration(labelText: 'Description'),
              ),
              const SizedBox(height: 20),
              ElevatedButton(
                onPressed: () => _updateLocation(context),
                child: const Text('Update'),
              ),
              const SizedBox(height: 20),
              ElevatedButton(
                onPressed: () => _deleteLocation(context),
                style: ElevatedButton.styleFrom(backgroundColor: Colors.red),
                child: const Text('Delete'),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Future<void> _updateLocation(BuildContext context) async {
    if (_formKey.currentState!.validate()) {
      await context.read<LocationProvider>().updateLocation({
        'id': widget.location['id'],
        'name': _nameController.text,
        'description': _descriptionController.text,
        'latitude': widget.location['latitude'],
        'longitude': widget.location['longitude'],
      });
      if (!context.mounted) return;
      Navigator.of(context).pop();
    }
  }

  Future<void> _deleteLocation(BuildContext context) async {
    // Show a confirmation dialog
    bool confirm = await showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Confirm Delete'),
        content: const Text('Are you sure you want to delete this location?'),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Delete'),
          ),
        ],
      ),
    );

    if (confirm) {
      if (!context.mounted) return;
      await context.read<LocationProvider>().deleteLocation(widget.location['id']);
      if (!context.mounted) return;
      Navigator.of(context).pop();
    }
  }
}