import 'package:flutter/widgets.dart' show WidgetExtension;
import 'package:flutter/material.dart' as fw hide WidgetExtension;

fw.Widget label(fw.Widget value) => value..label;
