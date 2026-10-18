import 'package:flutter/widgets.dart';

void unknownAlignment(dynamic alignment, double coordinate, Widget child) {
  Align(alignment: alignment, child: child);
  Align(alignment: Alignment(coordinate, 0), child: child);
  Align(alignment: Alignment(0, coordinate), child: child);
}

void inheritedValue(dynamic Alignment, Widget child) {
  Align(alignment: Alignment.center, child: child);
}
