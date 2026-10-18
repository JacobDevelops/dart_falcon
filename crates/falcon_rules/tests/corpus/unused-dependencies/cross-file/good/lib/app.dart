import 'package:from_lib/api.dart';
export 'package:from_export/api.dart';
import 'fallback.dart' if (dart.library.io) 'package:from_conditional/api.dart';
