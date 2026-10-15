import 'dart:async';
import 'dart:math';

void takeText(String text) {}

void takeNullable(String? text) {}

class Holder {
  String? name;

  int? size() => name!.length;

  void fill(String? value) {
    name = name!;
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

// Analyzer 3.13.5: Future.value retains its null assertion even when its
// instantiated parameter is FutureOr<int?>.
void futureValue(int? value) => Future<int>.value(value!);

// intl number_format.dart:120,136: generic arguments are inferred from the
// non-nullable operands, not widened by the nullable assignment context.
int? significantDigits;
void setSignificantDigits(int value) {
  significantDigits = min(significantDigits!, value);
  significantDigits = max(significantDigits!, value);
}

// Flutter badge.dart/dropdown_menu.dart/icon.dart: the enclosing nullable
// context is not the right operand's expected parameter type for `??`.
void acceptsNullable(int? value) {}
int? fallbackValue;
void nullableFallback(int? value) {
  acceptsNullable(value ?? fallbackValue!);
  int? local = value ?? fallbackValue!;
  significantDigits = value ?? fallbackValue!;
  print(local);
}


class Registration<T extends Object> {
  T? instance;
  final ready = Completer<T>();
}

// Future.value and Completer.complete deliberately retain their null checks,
// including a generic receiver obtained from an instance field.
void completeRegistration<T extends Object>(Registration<T> registration) {
  Future<T>.value(registration.instance!);
  registration.ready.complete(registration.instance!);
}
