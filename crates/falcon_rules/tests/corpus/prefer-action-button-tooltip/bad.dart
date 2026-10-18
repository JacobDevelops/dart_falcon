import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as m;

Widget missing() => FloatingActionButton(onPressed: () {}); /* expect: prefer-action-button-tooltip */
Widget empty() => FloatingActionButton(tooltip: '', onPressed: () {}); /* expect: prefer-action-button-tooltip */
Widget nullLabel() => new FloatingActionButton(tooltip: null, onPressed: () {}); /* expect: prefer-action-button-tooltip */
Widget prefixed() => m.FloatingActionButton.small(onPressed: () {}); /* expect: prefer-action-button-tooltip */
Widget extended() => FloatingActionButton.extended(label: Text('Add'), onPressed: () {}); /* expect: prefer-action-button-tooltip */
Widget icon() => IconButton(icon: Icon(Icons.add), onPressed: () {}); /* expect: prefer-action-button-tooltip */
Widget filled() => IconButton.filled(tooltip: r'', icon: Icon(Icons.add), onPressed: () {}); /* expect: prefer-action-button-tooltip */
Widget constant() => const FloatingActionButton(onPressed: null); /* expect: prefer-action-button-tooltip */
Widget adjacent() => FloatingActionButton(tooltip: '' '', onPressed: () {}); /* expect: prefer-action-button-tooltip */
Widget large() => FloatingActionButton.large(onPressed: () {}); /* expect: prefer-action-button-tooltip */
Widget outlined() => IconButton.outlined(icon: Icon(Icons.add), onPressed: () {}); /* expect: prefer-action-button-tooltip */
Widget tonal() => IconButton.filledTonal(icon: Icon(Icons.add), onPressed: () {}); /* expect: prefer-action-button-tooltip */
Widget wrapped() => Tooltip(
  message: 'Add',
  child: IconButton(icon: Icon(Icons.add), onPressed: () {}), /* expect: prefer-action-button-tooltip */
);
Widget iconLabel() => IconButton(icon: Icon(Icons.add, semanticLabel: 'Add'), onPressed: () {}); /* expect: prefer-action-button-tooltip */
