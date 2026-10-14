import 'targets/stub.dart'
    if (dart.library.io) 'targets/io.dart'
    if (dart.library.js_interop) 'targets/web.dart';
import 'targets/stub.dart'
    if (dart.library.io) './targets/io.dart'
    as second;
export 'targets/stub.dart'
    if (dart.library.js_interop) 'targets/web.dart';
// dart: libraries and other packages cannot be checked without the SDK and a
// package configuration, so they are never reported.
import 'targets/stub.dart'
    if (dart.library.io) 'dart:io'
    if (dart.library.js_interop) 'package:web/web.dart'
    as third;
// A missing default URI is the analyzer's uri_does_not_exist, not this rule.
import 'targets/missing_default.dart' as fourth;

void main() {}
