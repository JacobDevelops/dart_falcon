import 'package:flutter/widgets.dart' as fw;

bool first(Object value) => switch (value) {
  fw.WidgetKind.first => true,
  _ => false,
};
