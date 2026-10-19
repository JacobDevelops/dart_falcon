import 'package:flutter/widgets.dart' as f;
class Opacity { Opacity({double? opacity, dynamic child}); }
void example() {
  Opacity(opacity: .5, child: f.Image());
}
