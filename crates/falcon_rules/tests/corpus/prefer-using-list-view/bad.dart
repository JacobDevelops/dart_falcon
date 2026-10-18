import 'package:flutter/widgets.dart';
import 'package:flutter/widgets.dart' as f;
typedef V = SingleChildScrollView;
void callback() {}
void examples(Widget child, dynamic image, dynamic color, double size, String label) {
  SingleChildScrollView(child: Column()); /* expect: prefer-using-list-view */
  const SingleChildScrollView(child: Column(children: [])); /* expect: prefer-using-list-view */
  new SingleChildScrollView(child: Column(children: [child])); /* expect: prefer-using-list-view */
  f.SingleChildScrollView(child: f.Column()); /* expect: prefer-using-list-view */
  V(child: Column()); /* expect: prefer-using-list-view */
}
