import 'package:flutter/foundation.dart';
import 'package:flutter/foundation.dart' as foundation;

void boundaries(bool other) {
  if (kDebugMode) {
    void local() {
      print('nested function'); /* expect: avoid-print */
    }
    final callback = () => print('closure'); /* expect: avoid-print */
    [1].forEach(print); /* expect: avoid-print */
    local();
    callback();
  } else {
    print('else'); /* expect: avoid-print */
  }
  if ((kDebugMode)) print('parenthesized'); /* expect: avoid-print */
  if (kDebugMode && other) print('binary'); /* expect: avoid-print */
  if (foundation.kDebugMode) print('prefixed'); /* expect: avoid-print */
  print('unguarded'); /* expect: avoid-print */
}

void shadowed(bool kDebugMode) {
  if (kDebugMode) print('parameter'); /* expect: avoid-print */
}

class Shadowed {
  bool get kDebugMode => true;

  void run() {
    if (kDebugMode) print('member'); /* expect: avoid-print */
  }
}
