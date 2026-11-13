import 'package:flutter/widgets.dart' show WidgetExtension;
import 'package:flutter/material.dart' as fw hide WidgetExtension;

String label(Object value) => switch (value) {
  fw.Widget(:final label) => label,
  _ => '',
};
