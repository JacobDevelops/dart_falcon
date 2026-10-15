// Legacy typedef parameter lists are formal parameters, unlike Function types.
typedef void Legacy(int); /* expect: avoid-types-as-parameter-names */
typedef void Optional([int]); /* expect: avoid-types-as-parameter-names */
typedef void Named({int}); /* expect: avoid-types-as-parameter-names */
class Existing {}
typedef void LocalType(Existing); /* expect: avoid-types-as-parameter-names */
typedef void Modified(final Existing); /* expect: avoid-types-as-parameter-names */

void enclosing<T>() {
  void nested(T) {} /* expect: avoid-types-as-parameter-names */
}
void functionTyped(void callback(int)) {} /* expect: avoid-types-as-parameter-names */
