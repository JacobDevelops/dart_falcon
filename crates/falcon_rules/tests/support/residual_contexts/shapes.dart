class Parent {}
class Child extends Parent {}
int optional([Parent? value]) => 0;
int required(Child value) => 0;
int namedOptional({Parent? value}) => 0;
int namedRequired({required Child value}) => 0;
void check(bool condition) {
  final int Function(Child) positional = condition ? optional : required; /* expect: omit-local-variable-types */
  final int Function({required Child value}) named = condition ? namedOptional : namedRequired; /* expect: omit-local-variable-types */
}
