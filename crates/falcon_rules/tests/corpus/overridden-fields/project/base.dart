abstract class Base {
  int value = 0;
  abstract int abstractValue;
  int get computed => 0;
  static int staticValue = 0;
  covariant Object flexible = 0;
  int _private = 0;
}
