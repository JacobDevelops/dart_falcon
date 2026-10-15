enum Choice { one }
mixin Tag {}
class Box {}
mixin class Mixable {}
extension type Wrapped(int value) {}
typedef Alias = Choice;
void closures() {
  final a = (Choice) {};
  final b = (Tag) {};
  final c = (Box) {}; /* expect: avoid-types-as-parameter-names */
  final d = (Mixable) {}; /* expect: avoid-types-as-parameter-names */
  final e = (Wrapped) {}; /* expect: avoid-types-as-parameter-names */
  final f = (Alias) {}; /* expect: avoid-types-as-parameter-names */
  try {} catch (Choice) {}
  try {} catch (Tag) {}
}
class EnumParameter<Choice> {}
class MixinParameter<Tag> {}
class ClassParameter<Box> {} /* expect: avoid-types-as-parameter-names */
