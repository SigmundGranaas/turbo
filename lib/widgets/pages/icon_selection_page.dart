import 'package:flutter/material.dart';
import 'package:turbo/data/model/named_icon.dart';

import '../../data/icon_service.dart';

class IconSelectionPage extends StatefulWidget {
  final IconService iconService;
  const IconSelectionPage({super.key, required this.iconService});

  @override
  State<IconSelectionPage> createState() => _IconSelectionPageState();

  static Future<NamedIcon?> show(
      BuildContext context, IconService iconService) {
    return Navigator.of(context).push<NamedIcon?>(
      MaterialPageRoute(
          builder: (context) => IconSelectionPage(iconService: iconService)),
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
    _searchController.addListener(() {
      _filterIcons(_searchController.text);
    });
  }

  void _filterIcons(String query) {
    setState(() {
      _filteredIcons = _icons
          .where(
              (icon) => icon.title.toLowerCase().contains(query.toLowerCase()))
          .toList();
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Select an Icon'),
      ),
      body: Column(
        children: [
          Padding(
            padding: const EdgeInsets.all(16.0),
            child: SearchBar(
              controller: _searchController,
              hintText: 'Search icons...',
              leading: const Icon(Icons.search),
              trailing: [
                if (_searchController.text.isNotEmpty)
                  IconButton(
                    icon: const Icon(Icons.clear),
                    onPressed: () {
                      _searchController.clear();
                      FocusScope.of(context).unfocus();
                    },
                  )
              ],
            ),
          ),
          Expanded(
            child: _filteredIcons.isEmpty
                ? const Center(child: Text('No icons found.'))
                : LayoutBuilder(builder: (context, constraints) {
              if (constraints.maxWidth < 600) {
                return IconGrid(
                  icons: _filteredIcons,
                  onIconSelected: (icon) => Navigator.pop(context, icon),
                );
              } else {
                return IconList(
                  icons: _filteredIcons,
                  onIconSelected: (icon) => Navigator.pop(context, icon),
                );
              }
            }),
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

class IconGrid extends StatelessWidget {
  final List<NamedIcon> icons;
  final ValueChanged<NamedIcon> onIconSelected;

  const IconGrid({
    super.key,
    required this.icons,
    required this.onIconSelected,
  });

  @override
  Widget build(BuildContext context) {
    return GridView.builder(
      padding: const EdgeInsets.all(12),
      gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
        crossAxisCount: 4,
        childAspectRatio: 1,
        crossAxisSpacing: 8,
        mainAxisSpacing: 8,
      ),
      itemCount: icons.length,
      itemBuilder: (context, index) {
        return IconGridItem(
          icon: icons[index],
          onTap: () => onIconSelected(icons[index]),
        );
      },
    );
  }
}

class IconGridItem extends StatelessWidget {
  final NamedIcon icon;
  final VoidCallback onTap;

  const IconGridItem({
    super.key,
    required this.icon,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;

    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(12),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(icon.icon, size: 32),
          const SizedBox(height: 8),
          Text(
            icon.title,
            textAlign: TextAlign.center,
            style: textTheme.bodySmall,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
        ],
      ),
    );
  }
}

class IconList extends StatelessWidget {
  final List<NamedIcon> icons;
  final ValueChanged<NamedIcon> onIconSelected;

  const IconList({
    super.key,
    required this.icons,
    required this.onIconSelected,
  });

  @override
  Widget build(BuildContext context) {
    return ListView.builder(
      itemCount: icons.length,
      itemBuilder: (context, index) {
        final icon = icons[index];
        return ListTile(
          leading: Icon(icon.icon),
          title: Text(icon.title),
          onTap: () => onIconSelected(icon),
        );
      },
    );
  }
}