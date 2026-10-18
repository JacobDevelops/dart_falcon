import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as m;

Widget labeled() => FloatingActionButton(tooltip: 'Add', onPressed: () {});
Widget iconLabeled() => IconButton(tooltip: 'Add', icon: Icon(Icons.add), onPressed: () {});
Widget unknown(String? label) => FloatingActionButton(tooltip: label, onPressed: () {});
Widget interpolated(String label) => FloatingActionButton(tooltip: '$label', onPressed: () {});
Widget otherButton() => TextButton(child: Text('Add'), onPressed: () {});
Widget shadowed(dynamic FloatingActionButton) => FloatingActionButton(onPressed: () {});
Widget prefixShadow(dynamic m) => m.FloatingActionButton(onPressed: () {});
Widget nestedShadow(dynamic IconButton) => Tooltip(message: 'Add', child: IconButton(onPressed: () {}));
Widget localCustom() => CustomActionButton();
class CustomActionButton extends StatelessWidget {
  Widget build(BuildContext context) => const SizedBox();
}
