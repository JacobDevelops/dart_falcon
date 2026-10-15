// @dart = 3.13

import 'dart:io';
class C(IOSink sink) {
  // ignore: unused_field
  final IOSink _sink;

  this : this._sink = sink; /* expect: sort-constructors-first */
}
