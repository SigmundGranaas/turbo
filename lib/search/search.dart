import 'dart:convert';
import 'package:http/http.dart' as http;
import 'package:flutter/material.dart';

class SearchWidget extends StatefulWidget {
  final Function(double, double) onLocationSelected;

  const SearchWidget({super.key, required this.onLocationSelected});

  @override
  State<SearchWidget> createState() => _SearchWidgetState();
}

class _SearchWidgetState extends State<SearchWidget> {
  final TextEditingController _controller = TextEditingController();
  List<dynamic> _suggestions = [];

  @override
  Widget build(BuildContext context) {
    return Positioned(
      top: 40,
      left: 20,
      right: 20,
      child: Column(
        children: [
          Container(
            decoration: BoxDecoration(
              color: Colors.white,
              borderRadius: BorderRadius.circular(30),
              boxShadow: [
                BoxShadow(
                  color: Colors.black.withOpacity(0.1),
                  blurRadius: 10,
                  offset: const Offset(0, 5),
                ),
              ],
            ),
            child: TextField(
              controller: _controller,
              decoration: const InputDecoration(
                hintText: 'Søk',
                prefixIcon: Icon(Icons.search),
                border: InputBorder.none,
                contentPadding: EdgeInsets.symmetric(horizontal: 20, vertical: 15),
              ),
              onChanged: _onSearchChanged,
            ),
          ),
          if (_suggestions.isNotEmpty)
            Container(
              margin: const EdgeInsets.only(top: 5),
              decoration: BoxDecoration(
                color: Colors.white,
                borderRadius: BorderRadius.circular(10),
                boxShadow: [
                  BoxShadow(
                    color: Colors.black.withOpacity(0.1),
                    blurRadius: 10,
                    offset: const Offset(0, 5),
                  ),
                ],
              ),
              child: ListView.builder(
                shrinkWrap: true,
                itemCount: _suggestions.length,
                itemBuilder: (context, index) => _buildSuggestionItem(_suggestions[index]),
              ),
            ),
        ],
      ),
    );
  }

  void _onSearchChanged(String query) {
    if (query.length < 2) {
      setState(() => _suggestions = []);
      return;
    }
    _fetchSuggestions(query);
  }

  void _fetchSuggestions(String query) async {
    final url = Uri.parse('https://api.kartverket.no/stedsnavn/v1/navn?sok=${Uri.encodeComponent(query)}*&fuzzy=true&treffPerSide=5');
    try {
      final response = await http.get(url);
      if (response.statusCode == 200) {
        final data = json.decode(response.body);
        setState(() => _suggestions = data['navn']);
      }
    } catch (e) {
      print('Error fetching suggestions: $e');
    }
  }

  void _onSuggestionSelected(dynamic suggestion) {
    final point = suggestion['representasjonspunkt'];
    final east = point['øst'];
    final north = point['nord'];
    widget.onLocationSelected(east, north);
    setState(() => _suggestions = []);
    _controller.clear();
  }

  Widget _buildSuggestionItem(dynamic suggestion) {
    final name = suggestion['skrivemåte'];
    final type = suggestion['navneobjekttype'];
    final municipality = suggestion['kommuner'][0]['kommunenavn'];

    return ListTile(
      title: Text(name),
      subtitle: Row(
        children: [
          Text(type, style: TextStyle(fontWeight: FontWeight.bold)),
          SizedBox(width: 8),
          Text(municipality),
        ],
      ),
      onTap: () => _onSuggestionSelected(suggestion),
    );
  }
}