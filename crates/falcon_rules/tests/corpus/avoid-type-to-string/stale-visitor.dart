// Analyzer 3.13.5 quirk: class/mixin/extension visitor state is not reset.
class Type2 implements Type {}
void use(Object? value) {}
void dynamicTargets(dynamic value, Type2 typed) {
  value.toString(); /* expect: avoid-type-to-string */
  (typed as dynamic).toString(); /* expect: avoid-type-to-string */
  typed..toString(); /* expect: avoid-type-to-string */
}
void bounded<T extends Type2>(T value) {
  value.toString(); /* expect: avoid-type-to-string */
}
extension type Later(int value) {
  void describe() {
    toString(); /* expect: avoid-type-to-string */
  }
}
void nullable(Type2? value) {
  value?.toString();
  use(value?.toString);
}
class Type3 extends Type2 {
  String toString() => '';
  void describe() {
    super.toString(); /* expect: avoid-type-to-string */
  }
}
// Official mixinOnType_{explicitThis,implicitThis} ports are clean:
// Type.toString, not Object.toString, is the interface member.
mixin OnType on Type {
  late var implicit = toString();
  late var explicit = this.toString();
}
extension Reset on dynamic {}
void reset(dynamic value) { value.toString(); }
