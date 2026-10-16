import 'good_statics.dart' as p;

class StaticTargets {
  static int first = 0;
  static int second = 0;
  static State firstReceiver = State();
  static State secondReceiver = State();

  static int differentFields() {
    first++;
    return second;
  }

  static int differentReceivers() {
    firstReceiver.value = 1;
    return secondReceiver.value;
  }
}

class State {
  int value = 0;
}

extension StaticExtension on Object {
  static int first = 0;
  static int second = 0;
}

int extensionStatics() {
  StaticExtension.first = 1;
  return StaticExtension.second;
}

int prefixedClassStatics() {
  p.StaticTargets.first = 1;
  return p.StaticTargets.second;
}

int prefixedExtensionStatics() {
  p.StaticExtension.first = 1;
  return p.StaticExtension.second;
}

typedef Alias = StaticTargets;

int aliasStatics() {
  Alias.first = 1;
  return Alias.second;
}

int prefixedAliasStatics() {
  p.Alias.first = 1;
  return p.Alias.second;
}
