import 'dart:async';
import 'package:riverpod/riverpod.dart';
final controller = Provider((ref) {
  final resource = StreamController<int>(); /* expect: dispose-provider-resources */
  return resource;
});
final directController = Provider((ref) => StreamController<int>()); /* expect: dispose-provider-resources */
final timer = Provider((ref) => Timer(Duration.zero, () {})); /* expect: dispose-provider-resources */
