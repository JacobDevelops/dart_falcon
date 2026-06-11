import 'package:flutter/material.dart';

// Bad: using MediaQuery.of(context).size.width
class ResponsiveWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final width = MediaQuery.of(context).size.width; /* expect: prefer_dedicated_media_query_methods */
    return SizedBox(width: width);
  }
}

// Bad: using MediaQuery.of(context).size.height
class HeightWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final height = MediaQuery.of(context).size.height; /* expect: prefer_dedicated_media_query_methods */
    return SizedBox(height: height);
  }
}

// Bad: chaining size.width directly
class ContainerWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Container(
      width: MediaQuery.of(context).size.width, /* expect: prefer_dedicated_media_query_methods */
      child: Text('Full width'),
    );
  }
}

// Bad: accessing height in a calculation
class DynamicHeightWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final availableHeight = MediaQuery.of(context).size.height - 100; /* expect: prefer_dedicated_media_query_methods */
    return SizedBox(height: availableHeight);
  }
}

// Bad: using both width and height
class ResponsiveBoxWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final w = MediaQuery.of(context).size.width; /* expect: prefer_dedicated_media_query_methods */
    final h = MediaQuery.of(context).size.height; /* expect: prefer_dedicated_media_query_methods */
    return SizedBox(width: w, height: h);
  }
}
