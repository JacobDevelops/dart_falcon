import 'dart:async';
import 'src/a.dart';
import 'package:application/application.dart';
import 'package:other/src/internal.dart';
import 'src/f.dart' if (dart.library.io) 'package:app/src/f_io.dart';

// Exports are out of scope.
export 'package:app/src/a.dart';
