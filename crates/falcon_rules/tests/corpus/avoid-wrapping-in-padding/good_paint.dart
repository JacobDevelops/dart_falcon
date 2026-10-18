import 'package:flutter/widgets.dart';
void examples(dynamic paint) {
  Padding(padding: 1, child: Container(decoration: paint));
  Padding(padding: 1, child: Container(foregroundDecoration: paint));
  Padding(padding: 1, child: Container(transform: paint));
  Padding(padding: 1, child: Container(constraints: paint));
}
