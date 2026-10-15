class C { C([int? value]); }
class A {}
class B {}
void one(int value) {}
void two(int value, [int? extra]) {}
void three(int value, [int? extra, String? more]) {}
void optional([int? value]) {}
int left(A value, [int? tail]) => 0;
int right(B value) => 0;
int longer(A value, [int? tail, String? rest]) => 0;
void take(Function f) {}
void check(bool flag) {
  final void Function(int) conditional = flag ? one : two; /* expect: omit-local-variable-types */
  final List<void Function(int)> list = [one, two]; /* expect: omit-local-variable-types */
  final void Function(int, [int?]) prefix = flag ? two : three; /* expect: omit-local-variable-types */
  final C Function([int?]) constructor = C.new; /* expect: omit-local-variable-types */
  take(flag ? two : (int value) {}); /* expect: avoid-types-on-closure-parameters */
  // Different required counts do not provide a concrete function context.
  take(flag ? optional : (int value) {});
  // Incomparable parameter types force a synthesized common-prefix signature.
  final int Function(Never) mixed = flag ? left : right; /* expect: omit-local-variable-types */
  final int Function(Never) reverse = flag ? right : left; /* expect: omit-local-variable-types */
  final List<int Function(Never)> variants = [left, right]; /* expect: omit-local-variable-types */
  final int Function(A, [int?]) shorter = flag ? longer : left; /* expect: omit-local-variable-types */
}
