typedef Callback = void Function(int value);
class Widget {}
void callback(void Function(int) f) {}
void bad(void Function(int) int) {} /* expect: avoid-types-as-parameter-names */
void alias(Object Callback) {} /* expect: avoid-types-as-parameter-names */
void localType(Object Widget) {} /* expect: avoid-types-as-parameter-names */
