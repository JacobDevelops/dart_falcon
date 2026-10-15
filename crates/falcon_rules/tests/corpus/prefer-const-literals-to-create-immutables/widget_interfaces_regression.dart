import 'package:flutter/widgets.dart';
import 'package:flutter/foundation.dart' show DiagnosticLevel;

class WidgetInterface implements Widget {
  WidgetInterface(this.values);
  final List<int> values;
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
  @override
  String toString({DiagnosticLevel minLevel = DiagnosticLevel.info}) => 'WidgetInterface';
}

mixin Marker {}
class MixedInterface extends Object with Marker implements StatelessWidget {
  MixedInterface(this.values);
  final List<int> values;
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
  @override
  String toString({DiagnosticLevel minLevel = DiagnosticLevel.info}) => 'MixedInterface';
}

void interfaces() {
  print(WidgetInterface([1, 2]));
  print(MixedInterface([1, 2]));
}
