import 'bad_statics.dart' as p;

class StaticTargets {
  static int value = 0;
  static int update() {
    value++; /* expect: join-return-with-assignment */
    return value;
  }
}

extension StaticExtension on Object {
  static int value = 0;
}

int extensionStatics() {
  StaticExtension.value = 1; /* expect: join-return-with-assignment */
  return StaticExtension.value;
}

int prefixedClassStatics() {
  p.StaticTargets.value = 1; /* expect: join-return-with-assignment */
  return p.StaticTargets.value;
}

int prefixedExtensionStatics() {
  p.StaticExtension.value = 1; /* expect: join-return-with-assignment */
  return p.StaticExtension.value;
}

class LateFinalTargets {
  late final int value;
  int initialize() {
    value = 1; /* expect: join-return-with-assignment */
    return value;
  }
}

int initializeExternal(LateFinalTargets target) {
  target.value = 1; /* expect: join-return-with-assignment */
  return target.value;
}

typedef Alias = StaticTargets;

int aliasStatics() {
  Alias.value = 1; /* expect: join-return-with-assignment */
  return Alias.value;
}

int prefixedAliasStatics() {
  p.Alias.value = 1; /* expect: join-return-with-assignment */
  return p.Alias.value;
}

mixin Initialization on LateFinalTargets {
  int initializeFromConstraint() {
    value = 1; /* expect: join-return-with-assignment */
    return value;
  }
}

abstract class LateFinalView implements LateFinalTargets {}

int initializeInterface(LateFinalView target) {
  target.value = 1; /* expect: join-return-with-assignment */
  return target.value;
}
