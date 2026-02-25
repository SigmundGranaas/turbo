import 'package:flutter/material.dart';
import 'package:turbo/l10n/app_localizations.dart';
import '../models/named_icon.dart';
import '../data/icon_service.dart';

class IconSelectionPage extends StatefulWidget {
  const IconSelectionPage({super.key});

  @override
  State<IconSelectionPage> createState() => _IconSelectionPageState();

  static Future<NamedIcon?> show(BuildContext context) {
    return Navigator.of(context).push<NamedIcon?>(
      MaterialPageRoute(
          builder: (context) => const IconSelectionPage()),
    );
  }
}

class _IconSelectionPageState extends State<IconSelectionPage> {
  late List<NamedIcon> _icons;
  late List<NamedIcon> _filteredIcons;
  final TextEditingController _searchController = TextEditingController();
  final IconService _iconService = IconService();
  bool _isInitialized = false;

  @override
  void initState() {
    super.initState();
    _searchController.addListener(() {
      _filterIcons(_searchController.text);
    });
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (!_isInitialized) {
      _icons = _iconService.getAllIcons(context);
      _filteredIcons = _icons;
      _isInitialized = true;
    }
  }

  void _filterIcons(String query) {
    setState(() {
      _filteredIcons = _icons
          .where(
              (icon) => (icon.localizedTitle ?? icon.title).toLowerCase().contains(query.toLowerCase()))
          .toList();
    });
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.selectAnIcon),
      ),
      body: Column(
        children: [
          Padding(
            padding: const EdgeInsets.all(16.0),
            child: SearchBar(
              controller: _searchController,
              hintText: l10n.searchIcons,
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
                ? Center(child: Text(l10n.noIconsFound))
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
            icon.localizedTitle ?? icon.title,
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
          title: Text(icon.localizedTitle ?? icon.title),
          onTap: () => onIconSelected(icon),
        );
      },
    );
  }
}