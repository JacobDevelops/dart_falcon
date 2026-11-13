import 'package:flutter/widgets.dart' hide WidgetExtension, FrameworkBoxExtension; /* expect: unnecessary-flutter-imports */

int ordinary(int value) => value.abs();
Object dynamicAccess(dynamic value) => value.label;
