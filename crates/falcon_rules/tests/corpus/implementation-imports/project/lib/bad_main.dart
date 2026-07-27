import 'package:other/src/internal.dart'; /* expect: implementation-imports */
import 'package:current_package/src/own.dart';
import 'package:public/api.dart'
    if (dart.library.io) 'package:io_impl/src/io.dart' /* expect: implementation-imports */
    if (dart.library.html) 'package:web_impl/public.dart';

export 'package:other/src/exported.dart';

void main() {}
