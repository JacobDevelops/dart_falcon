import 'package:flutter/widgets.dart'; /* expect: unnecessary-flutter-imports */
import 'package:flutter/material.dart';
import 'package:flutter/widgets.dart' as fw; /* expect: unnecessary-flutter-imports */
import 'package:flutter/material.dart' as fw;

class Marker {
  const Marker(Object value);
}

@Widget()
@Widget.named()
@Marker(Widget.marker)
@fw.Widget.named()
class Example<T> {
  Widget field = const Widget();
  MaterialOnly material = MaterialOnly();
  Object staticTarget() => Widget.marker;
  Object tearOff() => Widget.new;
  Object namedTearOff() => Widget.named;
  Object namedConstructor() => Widget.named();
  Object typeArgument() => <Widget>[];
  Object cascade() => Widget()..toString();
  Object extensionOverride(Widget value) => WidgetExtension(value).label;
  Object enumValue() => WidgetKind.first;
  bool pattern(Object value) => switch (value) { Widget() => true, _ => false };
  fw.Widget prefixedType = fw.Widget.named();
  Object prefixedStatic() => fw.Widget.marker;
  Object prefixedExtension(Widget value) => fw.WidgetExtension(value).label;
  fw.MaterialOnly prefixedMaterial = fw.MaterialOnly();
  Object identifier() => Widget;
  bool typeTest(Object value) => value is Widget;
  Widget cast(Object value) => value as Widget;
  List<T> genericArgument(T value) => <T>[value];
}

mixin GenericMixin<T> {
  List<T> values(T value) => <T>[value];
}
mixin class GenericMixinClass<T> {
  List<T> values(T value) => <T>[value];
}
class GenericAlias<T> = Object with GenericMixin<T>;
enum GenericEnum<T> {
  first<Never>();
  List<T> values(T value) => <T>[value];
}
extension GenericExtension<T> on List<T> {
  List<T> values(T value) => <T>[value];
}
extension type GenericExtensionType<T>(List<T> value) {
  List<T> values(T item) => <T>[item];
}
List<T> genericFunction<T>(T value) => <T>[value];
class GenericMethods {
  List<T> method<T>(T value) => <T>[value];
}
typedef GenericAliasType<T> = List<T>;
typedef GenericLegacyAlias<T>(T value);
typedef GenericFunctionType = List<T> Function<T>(T value);
final genericClosure = <T>(T value) => <T>[value];
void genericFormal(T callback<T>(T value)) {}
void genericLocalFunction() {
  List<T> local<T>(T value) => <T>[value];
}
