// @dart=2.16
// Super parameters are unavailable before Dart 2.17.
class Base {
  Base(int value);
}
class Old extends Base {
  Old(int value) : super(value);
}
