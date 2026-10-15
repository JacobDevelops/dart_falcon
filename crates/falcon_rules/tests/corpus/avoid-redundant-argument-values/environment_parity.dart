// Dart analyzer 3.13.5 keeps js_util/js_interop bool values unknown even
// with explicit defaults. Other names use the empty CLI declared-variable map.
const web = bool.fromEnvironment('dart.library.js_util');
const interop = bool.fromEnvironment('dart.library.js_interop');
const libraryName = 'dart.library.js_util';
void yes({bool value = true}) {}
void no({bool value = false}) {}
void text({String value = ''}) {}
void number({int value = 0}) {}
void cursor({int value = 2}) {}
void unknownDefault({bool value = web}) {}

void checkEnvironment() {
  yes(value: web);
  no(value: web);
  yes(value: interop);
  no(value: interop);
  no(value: const bool.fromEnvironment(libraryName));
  yes(value: const bool.fromEnvironment(libraryName, defaultValue: true));
  no(value: const bool.fromEnvironment('dart.library.js_interop', defaultValue: true));
  cursor(value: web ? 1 : 2);
  unknownDefault(value: false);
  no(value: /* expect: avoid-redundant-argument-values */ const bool.fromEnvironment('user.option'));
  no(value: /* expect: avoid-redundant-argument-values */ const bool.fromEnvironment('dart.library.io'));
  yes(value: /* expect: avoid-redundant-argument-values */ const bool.fromEnvironment('dart.library.io', defaultValue: true));
  no(value: /* expect: avoid-redundant-argument-values */ const bool.hasEnvironment('dart.library.js_util'));
  text(value: /* expect: avoid-redundant-argument-values */ const String.fromEnvironment('dart.library.js_util'));
  number(value: /* expect: avoid-redundant-argument-values */ const int.fromEnvironment('dart.library.js_interop'));
}
