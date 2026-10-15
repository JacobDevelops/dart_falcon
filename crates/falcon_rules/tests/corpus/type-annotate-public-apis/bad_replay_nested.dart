// @dart=3.12
class C {
  final void Function(int) f;
  C(void this.f(final p)); /* expect: type-annotate-public-apis */
}
