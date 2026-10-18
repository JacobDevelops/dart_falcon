void f() {
  consume('unique');
  trace('ignored');
  consume('');
  consume('');
  final value = 'initializer';
}
