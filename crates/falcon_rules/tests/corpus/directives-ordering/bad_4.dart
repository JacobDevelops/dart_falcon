// Each section is sorted alphabetically.
import 'dart:math';
import 'dart:async'; /* expect: directives-ordering */

import 'package:path/path.dart';
import 'package:collection/collection.dart'; /* expect: directives-ordering */

import 'b.dart';
import 'a.dart'; /* expect: directives-ordering */

export 'package:z/z.dart';
export 'package:y/y.dart'; /* expect: directives-ordering */
