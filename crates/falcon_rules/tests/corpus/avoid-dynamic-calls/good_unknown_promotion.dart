import 'package:unavailable/types.dart';

void check(dynamic value) {
  if (value is MissingType) {
    value.member();
  }
}
