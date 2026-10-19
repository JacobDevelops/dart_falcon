import 'dart:core' if (dart.library.io == 'true') 'dart:core';

void f() {
  consume('unique');
  trace('ignored');
  consume('');
  consume('');
  final value = 'initializer';
  final greeting = 'named only';
  final otherGreeting = 'named only';
  if (value == 'true') {}
  consume('dart:core');
}
