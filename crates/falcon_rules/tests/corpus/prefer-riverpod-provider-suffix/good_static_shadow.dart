import 'package:riverpod/riverpod.dart';

class Example {
  static int Provider(Object create) => 0;
  static final count = Provider((ref) => 0);
}
