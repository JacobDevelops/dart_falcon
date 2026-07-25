import 'package:flutter/material.dart';

// Bad: using MediaQuery.of(context).size.width
class ResponsiveWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final width = MediaQuery.of(context).size.width; /* expect: prefer-dedicated-media-query-methods */
    return SizedBox(width: width);
  }
}

// Bad: using MediaQuery.of(context).size.height
class HeightWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final height = MediaQuery.of(context).size.height; /* expect: prefer-dedicated-media-query-methods */
    return SizedBox(height: height);
  }
}

// Bad: chaining size.width directly
class ContainerWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Container(
      width: MediaQuery.of(context).size.width, /* expect: prefer-dedicated-media-query-methods */
      child: Text('Full width'),
    );
  }
}

// Bad: accessing height in a calculation
class DynamicHeightWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final availableHeight = MediaQuery.of(context).size.height - 100; /* expect: prefer-dedicated-media-query-methods */
    return SizedBox(height: availableHeight);
  }
}

// Bad: using both width and height
class ResponsiveBoxWidget extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final w = MediaQuery.of(context).size.width; /* expect: prefer-dedicated-media-query-methods */
    final h = MediaQuery.of(context).size.height; /* expect: prefer-dedicated-media-query-methods */
    return SizedBox(width: w, height: h);
  }
}

// Regression: the violation must still be found inside Dart 3 containers
// (pattern declaration, pattern assignment, labeled statement, switch
// expression, collection if/spread, record field, assert).
void containersRegression(int rcount, BuildContext context) {
  final (ra, _) = (MediaQuery.of(context).size.width, 0); /* expect: prefer-dedicated-media-query-methods */
  lbl: {
    final rb = MediaQuery.of(context).size.width; /* expect: prefer-dedicated-media-query-methods */
    print(rb);
  }
  final rc = switch (rcount) {
    0 => MediaQuery.of(context).size.width, /* expect: prefer-dedicated-media-query-methods */
    _ => null,
  };
  final rd = switch (MediaQuery.of(context).size.width) { /* expect: prefer-dedicated-media-query-methods */
    _ => 0,
  };
  final re = [if (rcount > 0) MediaQuery.of(context).size.width]; /* expect: prefer-dedicated-media-query-methods */
  final rf = [...[MediaQuery.of(context).size.width]]; /* expect: prefer-dedicated-media-query-methods */
  final rg = (p: MediaQuery.of(context).size.width, q: 0); /* expect: prefer-dedicated-media-query-methods */
  print([ra, rc, rd, re, rf, rg]);
}
