late int top = 2, second = 3; /* expect: unnecessary-late */
class Example {
  static late int field = 1; /* expect: unnecessary-late */
  void method() {
    late int local = 3;
    print(local);
  }
}
