// Declaration-only SDK excerpt for SDK-free semantic tests and corpus tools.
library dart.typed_data;
import 'dart:core';
abstract class Uint8List implements List<int> {
  factory Uint8List(int length);
}
abstract class Uint16List implements List<int> {
  factory Uint16List(int length);
}
abstract class BytesBuilder {
  factory BytesBuilder({bool copy = true});
  void add(List<int> bytes);
  Uint8List toBytes();
}
