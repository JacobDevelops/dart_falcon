import 'package:flutter/material.dart';

// Good: using MediaQuery.sizeOf() for width
class ResponsiveWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final width = MediaQuery.sizeOf(context).width;
    return SizedBox(width: width);
  }
}

// Good: using MediaQuery.sizeOf() for height
class HeightWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final height = MediaQuery.sizeOf(context).height;
    return SizedBox(height: height);
  }
}

// Good: using dedicated extension method (if available)
class ContainerWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Container(
      width: MediaQuery.sizeOf(context).width,
      child: Text('Full width'),
    );
  }
}

// Good: using other MediaQuery methods correctly
class PaddingWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final devicePadding = MediaQuery.of(context).padding;
    return Padding(
      padding: devicePadding,
      child: Text('Safe area'),
    );
  }
}

// Good: using sizeOf for dynamic height calculation
class DynamicHeightWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final availableHeight = MediaQuery.sizeOf(context).height - 100;
    return SizedBox(height: availableHeight);
  }
}

// Good: using sizeOf for both dimensions
class ResponsiveBoxWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final size = MediaQuery.sizeOf(context);
    return SizedBox(width: size.width, height: size.height);
  }
}

// Good: using dedicated MediaQuery methods
class DeviceInfoWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final textScaleFactor = MediaQuery.of(context).textScaleFactor;
    final orientation = MediaQuery.of(context).orientation;
    return Column(
      children: [
        Text('Scale: $textScaleFactor'),
        Text('Orientation: $orientation'),
      ],
    );
  }
}
