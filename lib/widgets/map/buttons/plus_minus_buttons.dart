import 'package:flutter/material.dart';

class PlusMinusButtons extends StatelessWidget {
  final Function() onZoomIn;
  final Function() onZoomOut;


  const PlusMinusButtons({super.key, required this.onZoomIn, required this.onZoomOut});

  @override
  Widget build(BuildContext context) {
    final width = MediaQuery.of(context).size.width;
    final isMobile = width < 600;

    // Define sizes based on device type
    final paddingSize = isMobile ? 4.0 : 8.0;
    final spacingHeight = isMobile ? 4.0 : 8.0;

    return Card(
      elevation: 4,
      shape:  RoundedRectangleBorder(borderRadius: BorderRadius.circular(25)),
      child: Padding(
        padding: EdgeInsets.all(paddingSize),
        child: Column(
          children: [
            IconButton(
              icon: Icon(Icons.add, color:  Colors.blueGrey[800]!),
              onPressed: onZoomIn,
            ),
            SizedBox(height: spacingHeight),
            IconButton(
              icon: Icon(Icons.remove, color: Colors.blueGrey[800]!),
              onPressed: onZoomOut
            ),
          ],
        ),
      ),
    );
  }
}
