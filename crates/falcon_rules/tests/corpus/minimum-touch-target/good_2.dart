import 'package:flutter/material.dart' as f;
class SizedBox { SizedBox({double? width, Object? child}); }
class IconButton { IconButton(); }
void custom() {
  SizedBox(width: 1, child: f.IconButton());
  f.SizedBox(width: 1, child: IconButton());
}
