import 'package:flutter/widgets.dart' show WidgetExtension; /* expect: unnecessary-flutter-imports */
import 'package:flutter/material.dart' as fw hide WidgetExtension;

Future<fw.Widget> waitFor(fw.Widget value) async => await value;
