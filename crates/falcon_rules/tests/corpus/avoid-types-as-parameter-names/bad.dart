// @dart = 3.13

typedef Callback = void Function(int value);
class Widget {}
void primitive(int) {} /* expect: avoid-types-as-parameter-names */
void alias(Callback) {} /* expect: avoid-types-as-parameter-names */
void localType(Widget) {} /* expect: avoid-types-as-parameter-names */
void shadowType<Widget>() {} /* expect: avoid-types-as-parameter-names */
void outer<T>() {
  void inner<T>() {} /* expect: avoid-types-as-parameter-names */
  final Object Function<T>(T) callback = <T>(T value) => value; /* expect: avoid-types-as-parameter-names */ /* expect: avoid-types-as-parameter-names */
}
void catches() {
  try {} catch (Widget) {} /* expect: avoid-types-as-parameter-names */
}

class Primary(Widget); /* expect: avoid-types-as-parameter-names */
typedef void LegacyTypeName(Widget); /* expect: avoid-types-as-parameter-names */
void nestedImplicit(void cb(Widget)) {} /* expect: avoid-types-as-parameter-names */
