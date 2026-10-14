class Deprecated {
  const Deprecated(String message);
}
@Deprecated('Not dart:core metadata')
void customAnnotatedFunction() {}
void useCustomAnnotation() { customAnnotatedFunction(); }
