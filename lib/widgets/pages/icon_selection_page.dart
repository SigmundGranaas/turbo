import 'package:flutter/material.dart';
import 'package:map_app/data/model/named_icon.dart';

import '../../data/icon_service.dart';

class IconSelectionPage extends StatefulWidget {
  final IconService iconService;
  const IconSelectionPage({super.key, required this.iconService});

  @override
  State<IconSelectionPage> createState() => _IconSelectionPageState();

  static Future<NamedIcon?> show(BuildContext context, IconService iconService) {
    return Navigator.of(context).push<NamedIcon?>(
      MaterialPageRoute(builder: (context) => IconSelectionPage(iconService: iconService)),
    );
  }
}

class _IconSelectionPageState extends State<IconSelectionPage> {
  late List<NamedIcon> _icons;
  late List<NamedIcon> _filteredIcons;
  final TextEditingController _searchController = TextEditingController();

  @override
  void initState() {
    super.initState();
    _icons = widget.iconService.getAllIcons();
    _filteredIcons = _icons;
  }

  void _filterIcons(String query) {
    setState(() {
      _filteredIcons = _icons
          .where((icon) =>
          icon.title.toLowerCase().contains(query.toLowerCase()))
          .toList();
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        leading: IconButton(
          icon: const Icon(Icons.arrow_back),
          onPressed: () => Navigator.pop(context),
        ),
        title: const Text('Ikoner', style: TextStyle(fontWeight: FontWeight.bold),),
        actions: [
          IconButton(
            icon: const Icon(Icons.close),
            onPressed: () => Navigator.pop(context),
          ),
        ],
      ),
      body: Column(
        children: [
          IconSearchBar(
            controller: _searchController,
            onChanged: _filterIcons,
          ),
          Expanded(
            child: _filteredIcons.isEmpty
                ? const Center(child: Text('Ingen resultater'))
                : IconGrid(
              icons: _filteredIcons,
              onIconSelected: (icon) => Navigator.pop(context, icon),
            ),
          ),
        ],
      ),
    );
  }

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }
}

class IconSearchBar extends StatelessWidget {
  final TextEditingController controller;
  final ValueChanged<String> onChanged;

  const IconSearchBar({
    super.key,
    required this.controller,
    required this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.all(16.0),
      child: TextField(
        controller: controller,
        decoration: InputDecoration(
          hintText: 'Søk på ikoner',
          prefixIcon: const Icon(Icons.search),
          filled: true,
          fillColor: Colors.grey[200],
          border: OutlineInputBorder(
            borderRadius: BorderRadius.circular(30),
            borderSide: BorderSide.none,
          ),
        ),
        onChanged: onChanged,
      ),
    );
  }
}

class IconGrid extends StatelessWidget {
  final List<NamedIcon> icons;
  final ValueChanged<NamedIcon> onIconSelected;
  final double itemSize = 64.0;
  final double iconSize = 32.0;

  const IconGrid({
    super.key,
    required this.icons,
    required this.onIconSelected,
  });

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final int crossAxisCount = (constraints.maxWidth / itemSize).floor();

        return GridView.builder(
          padding: const EdgeInsets.all(8),
          gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
            crossAxisCount: crossAxisCount,
            childAspectRatio: 1,
            crossAxisSpacing: 0,
            mainAxisSpacing: 0,
          ),
          itemCount: icons.length,
          itemBuilder: (context, index) {
            return IconGridItem(
              icon: icons[index],
              itemSize: itemSize,
              iconSize: iconSize,
              onTap: () => onIconSelected(icons[index]),
            );
          },
        );
      },
    );
  }
}

class IconGridItem extends StatelessWidget {
  final NamedIcon icon;
  final double itemSize;
  final double iconSize;
  final VoidCallback onTap;

  const IconGridItem({
    super.key,
    required this.icon,
    required this.itemSize,
    required this.iconSize,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      child: Semantics(
        button: true,
        label: 'Select ${icon.title} icon',
        child: Material(
          color: Colors.transparent,
          child: InkWell(
            onTap: onTap,
            hoverColor: Colors.blue.withValues(alpha: 0.1),
            borderRadius: BorderRadius.circular(10),
            child: Container(
              width: itemSize,
              height: itemSize,
              padding: const EdgeInsets.all(4),
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Icon(
                    icon.icon,
                    size: iconSize,
                    color: Colors.grey[600],
                  ),
                  const SizedBox(height: 2),
                  Text(
                    icon.title,
                    textAlign: TextAlign.center,
                    style: const TextStyle(fontSize: 10),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}