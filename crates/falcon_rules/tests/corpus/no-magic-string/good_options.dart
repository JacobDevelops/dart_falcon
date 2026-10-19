void f() {
  consume('unique');
  trace('ignored');
  consume('');
  consume('');
  final value = 'initializer';
  final greeting = 'named only';
  final otherGreeting = 'named only';
}
