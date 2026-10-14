import 'dart:async';
import 'package:app/src/a.dart';
import 'package:collection/collection.dart';
import 'package:app/src/f.dart'
    if (dart.library.io) 'src/f_io.dart'
    if (dart.library.js_interop) 'src/f_web.dart';

// Exports are out of scope.
export 'src/a.dart';
