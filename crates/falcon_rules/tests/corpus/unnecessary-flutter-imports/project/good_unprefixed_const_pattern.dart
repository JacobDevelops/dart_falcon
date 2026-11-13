import 'package:flutter/widgets.dart';

bool constant(Object value) => switch (value) {
  WidgetKind.first => true,
  frameworkMarker => false,
  _ => false,
};
