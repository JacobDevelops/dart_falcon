import 'package:flutter/widgets.dart';
import 'package:flutter/widgets.dart' as f;
typedef P = Padding;
void callback() {}
void examples(Widget child, dynamic image, dynamic color, double size, String label) {
  Padding(padding: 1, child: SizedBox());
  Padding(padding: 1, child: Container(padding: 2));
  Padding(padding: 1, child: Container(width: 20));
  Padding(padding: 1, child: Container(margin: 2));
  Padding(padding: 1, child: Container(color: color));
}
