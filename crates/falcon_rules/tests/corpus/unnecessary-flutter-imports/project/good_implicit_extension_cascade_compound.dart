import 'package:flutter/widgets.dart' show WidgetExtension;
import 'package:flutter/material.dart' as fw hide WidgetExtension;

class Holder { fw.Widget value = const fw.Widget(); }
void subtract(Holder holder) { holder..value -= const fw.Widget(); }
