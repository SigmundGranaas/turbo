import 'package:flutter/material.dart';

class PlusMinusButtons extends StatelessWidget {
  final Function() onZoomIn;
  final Function() onZoomOut;


  const PlusMinusButtons({super.key, required this.onZoomIn, required this.onZoomOut});

  @override
  Widget build(BuildContext context) {
    return Card(
      elevation: 4,
      child: Padding(
        padding: const EdgeInsets.all(8.0),
        child: Column(
          children: [
            const SizedBox(height: 8),
            IconButton(
              icon: const Icon(Icons.add),
              onPressed: onZoomIn
            ),
            const SizedBox(height: 8),
            IconButton(
              icon: const Icon(Icons.remove),
              onPressed: onZoomOut
            ),
          ],
        ),
      ),
    );
  }
}
