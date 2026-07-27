import 'dart:io';
import 'package:sample_app/internal.dart';
import 'package:declared/declared.dart';
import 'package:missing/missing.dart'; /* expect: depend-on-referenced-packages */
import 'package:dev_only/dev_only.dart'; /* expect: depend-on-referenced-packages */
import 'local.dart';
export 'package:export_missing/api.dart'; /* expect: depend-on-referenced-packages */
export 'package:declared/stub.dart'
    if (dart.library.io) 'package:conditional_missing/io.dart'; /* expect: depend-on-referenced-packages */

void main() {
  stdout.write('ok');
}
