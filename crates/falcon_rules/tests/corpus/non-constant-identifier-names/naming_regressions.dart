int value_ = 0;
int value_4 = 0;
int _4 = 0; /* expect: non-constant-identifier-names */
int bad_name = 0; /* expect: non-constant-identifier-names */

class Fields {
  final int Bad_Field; /* expect: non-constant-identifier-names */
  Fields(this.Bad_Field);
}

typedef Callback = void Function(int Bad_Param); /* expect: non-constant-identifier-names */
typedef LegacyCallback(int Bad_Param); /* expect: non-constant-identifier-names */
typedef GoodCallback = void Function(int goodParam);

class Wildcards {
  Wildcards.__();
  set value(int __) {}
}

void catchWildcards() {
  try {} catch (_, __) {}
}

extension type Representation.__(int value) {} /* expect: non-constant-identifier-names */
