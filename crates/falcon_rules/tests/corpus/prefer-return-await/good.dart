import 'dart:async';
Future<int> fetch() async => 1;
Future<int> forwarding() => fetch();
Future<int> outside() async { return fetch(); }
Future<int> arrow() async => fetch();
Future<int> awaited() async {
  try { return await fetch(); } catch (_) { return 0; }
}
Future<int> scalar() async {
  try { return 1; } catch (_) { return 0; }
}
Future<int> finalizerOnly() async {
  try { return fetch(); } finally { print('done'); }
}
Future<int> finalizerReturn() async {
  try { return 1; } catch (_) { return 0; }
  finally { return fetch(); }
}
Future<int> catchReturn() async {
  try { return 1; } catch (_) { return fetch(); }
}
Future<int> nested() async {
  try {
    Future<int> inner() async { return fetch(); }
    return await inner();
  } catch (_) { return 0; }
}
Future<dynamic> unknown(dynamic value) async {
  try { return value; } catch (_) { return 0; }
}
