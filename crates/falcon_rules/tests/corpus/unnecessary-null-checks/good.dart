import 'dart:async';

void takeText(String text) {}

void takeNullable(String? text) {}

class Holder {
  String? _cache;
  String? name;

  int? size() => name!.length;

  void fill(String? value) {
    _cache = value!;
  }
}

int count(int? value) => value!;

String? Function() closureOf(String? s) => () => s!;

String firstOr(List<String?> xs, bool Function(String?) p, String? fallback) =>
    xs.firstWhere(p, orElse: () => fallback!)!;

void main(String? text, int? number, (int?, int) pair) async {
  takeText(text!);
  int copy = number!;
  String? local;
  local = text!;
  final future = Future<int?>.value(number!);
  final completer = Completer<int?>();
  completer.complete(number!);
  final list = [number!];
  final (int a, int b) = (pair.$1!, pair.$2);
  var (c!, d) = pair;
  takeNullable(text);
  print([copy, local, future, list, a, b, c, d, count(1)]);
}
