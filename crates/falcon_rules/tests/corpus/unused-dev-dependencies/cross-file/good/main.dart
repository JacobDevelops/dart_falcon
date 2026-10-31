export 'package:exported/api.dart';
import 'package:conditional_default/api.dart'
  if (dart.library.io) 'package:conditional_io/api.dart';
void main() {}
