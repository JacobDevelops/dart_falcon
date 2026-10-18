import 'package:flutter/widgets.dart';
import 'package:flutter/widgets.dart' as f;
typedef I = Image;
void callback() {}
void examples(Widget child, dynamic image, dynamic color, double size, String label) {
  Opacity(opacity: .5, child: SizedBox());
  Image(image: image, opacity: image);
  Opacity(opacity: .5);
  Opacity(opacity: .5, child: Container(child: Image()));
  Opacity(opacity: .5, child: child);
}
