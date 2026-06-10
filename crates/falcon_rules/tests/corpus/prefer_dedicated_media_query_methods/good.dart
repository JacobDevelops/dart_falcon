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
