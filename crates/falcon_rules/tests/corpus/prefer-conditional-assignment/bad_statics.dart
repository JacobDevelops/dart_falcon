import 'bad_statics.dart' as p;

class StaticTargets {
  static int? value;
  static void update() {
    if (value == null) value = 1; /* expect: prefer-conditional-assignment */
  }
}

extension StaticExtension on Object {
  static int? value;
}

void extensionStatics() {
  if (StaticExtension.value == null) StaticExtension.value = 1; /* expect: prefer-conditional-assignment */
}

void prefixedClassStatics() {
  if (p.StaticTargets.value == null) p.StaticTargets.value = 1; /* expect: prefer-conditional-assignment */
}

void prefixedExtensionStatics() {
  if (p.StaticExtension.value == null) p.StaticExtension.value = 1; /* expect: prefer-conditional-assignment */
}

class LateFinalTargets {
  late final int? value;
  void initialize() {
    if (value == null) value = 1; /* expect: prefer-conditional-assignment */
  }
}

void initializeExternal(LateFinalTargets target) {
  if (target.value == null) target.value = 1; /* expect: prefer-conditional-assignment */
}

typedef Alias = StaticTargets;

void aliasStatics() {
  if (Alias.value == null) Alias.value = 1; /* expect: prefer-conditional-assignment */
}

void prefixedAliasStatics() {
  if (p.Alias.value == null) p.Alias.value = 1; /* expect: prefer-conditional-assignment */
}

mixin Initialization on LateFinalTargets {
  void initializeFromConstraint() {
    if (value == null) value = 1; /* expect: prefer-conditional-assignment */
  }
}

abstract class LateFinalView implements LateFinalTargets {}

void initializeInterface(LateFinalView target) {
  if (target.value == null) target.value = 1; /* expect: prefer-conditional-assignment */
}
