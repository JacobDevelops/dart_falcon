import 'good_statics.dart' as p;

class StaticTargets {
  static int? first;
  static int? second;
  static State firstReceiver = State();
  static State secondReceiver = State();

  static void differentFields() {
    if (first == null) second = 1;
  }

  static void differentReceivers() {
    if (firstReceiver.value == null) secondReceiver.value = 1;
  }
}

class State {
  int? value;
}

extension StaticExtension on Object {
  static int? first;
  static int? second;
}

void extensionStatics() {
  if (StaticExtension.first == null) StaticExtension.second = 1;
}

void prefixedClassStatics() {
  if (p.StaticTargets.first == null) p.StaticTargets.second = 1;
}

void prefixedExtensionStatics() {
  if (p.StaticExtension.first == null) p.StaticExtension.second = 1;
}

typedef Alias = StaticTargets;

void aliasStatics() {
  if (Alias.first == null) Alias.second = 1;
}

void prefixedAliasStatics() {
  if (p.Alias.first == null) p.Alias.second = 1;
}
