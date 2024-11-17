import 'package:flutter/material.dart';

class GoBackButton extends StatelessWidget {

  const GoBackButton({super.key});

  @override
  Widget build(BuildContext context) {
    return Card(
        elevation: 4,
        shape: const CircleBorder(),
        child: Padding(
          padding: const EdgeInsets.all(8.0),
          child: IconButton(
            icon: const Icon(Icons.arrow_back),
            onPressed: () => Navigator.of(context).pop(),
            tooltip: 'Go Back',
          ),
        )
    );
  }
}