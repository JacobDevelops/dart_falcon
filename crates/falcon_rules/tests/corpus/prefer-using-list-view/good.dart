import 'package:flutter/widgets.dart';
import 'package:flutter/widgets.dart' as f;
typedef V = SingleChildScrollView;
void callback() {}
void examples(Widget child, dynamic image, dynamic color, double size, String label) {
  ListView(children: [child]);
  SingleChildScrollView(child: Row());
  SingleChildScrollView(child: child);
  Column(children: [child]);
  SingleChildScrollView();
}
