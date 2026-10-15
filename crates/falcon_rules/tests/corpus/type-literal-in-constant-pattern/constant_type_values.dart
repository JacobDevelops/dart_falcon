const topType = int;
const Object widenedType = int;

class Types {
  static const ty = String;
  static const Object widened = String;
}

void checkConstantTypes(Object? value) {
  const localType = bool;
  if (value case topType) {} /* expect: type-literal-in-constant-pattern */
  if (value case Types.ty) {} /* expect: type-literal-in-constant-pattern */
  if (value case localType) {} /* expect: type-literal-in-constant-pattern */
  if (value case widenedType) {}
  if (value case Types.widened) {}
  if (value case const (int)) {}
}

void matchedType(Type value) {
  if (value case topType) {}
}

class MemberShadow {
  static const num int = 1;
  void check(Object? value) {
    if (value case int) {}
  }
}
