import 'package:flutter/widgets.dart' show WidgetExtension;
import 'package:flutter/material.dart' as fw hide WidgetExtension;

String callable(fw.Widget value) => value();
String method(fw.Widget value) => value.call();
String? nullable(fw.Widget? value) => value?.label;
void setter(fw.Widget value) { value.label = 'changed'; }
void compound(fw.Widget value) { value.label += '!'; }
void cascade(fw.Widget value) { value..label += '!'; }
void increments(fw.Widget value) { value.count++; ++value.count; }
void indexSetter(fw.Widget value) { value[0] = value; }
void nullableCascade(fw.Widget? value) { value?..label; }
fw.Widget? nullableIndex(fw.Widget? value) => value?[0];
