void outer() {
  void first() {} /* expect: avoid-local-functions */
  int second() => 2; /* expect: avoid-local-functions */
  if (true) {
    void third() {} /* expect: avoid-local-functions */
  }
  final callback = () {
    void fourth() {} /* expect: avoid-local-functions */
  };
}
class Example {
  void method() {
    T identity<T>(T value) => value; /* expect: avoid-local-functions */
  }
}
