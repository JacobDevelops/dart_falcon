void consume<T>(T value) {}
class Holder<T> {
  late T field;
  void named({required T value}) {}
  void fieldAssignment(T? value) {
    this.field = value!; /* expect: null-check-on-nullable-type-parameter */
  }
  void namedArgument(T? value) {
    named(value: value!); /* expect: null-check-on-nullable-type-parameter */
  }
  void sameField() {
    this.field = this.field!;
  }
}
void genericArgument<T>(T? value) {
  consume(value!); /* expect: null-check-on-nullable-type-parameter */
}
