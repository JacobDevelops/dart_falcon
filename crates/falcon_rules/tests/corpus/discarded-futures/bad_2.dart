// `Future` resolves through the implicit `dart:core` import.
Future<void> write() async {}

void flush() {
  write(); /* expect: discarded-futures */
}
