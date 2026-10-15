const zero = 0;
const one = 1;
class Equality {
  const Equality();
  @override
  bool operator ==(Object other) => other is Equality;
  @override
  int get hashCode => 0;
}
const equality = Equality();
void arithmetic<T>() {
  final divide = 1 ~/ zero;
  final remainder = 1 % zero;
  final floatingDivide = 1.0 ~/ zero;
  final leftShift = 1 << -1;
  final rightShift = one >> -1;
  final instances = const Equality() == const Equality();
  final references = equality == equality;
  final duration = const Duration(seconds: 1) == const Duration(seconds: 1);
  final type = T;
  const localZero = 0;
  const localOne = 1;
  final localDivide = 1 ~/ localZero;
  final localShift = localOne << -1;
  final valid = localOne + one; /* expect: prefer-const-declarations */
  print([divide, remainder, floatingDivide, leftShift, rightShift, instances,
    references, duration, type, localDivide, localShift, valid]);
}

class StaticConstants {
  static const count = 2;
  static void values() {
    final doubled = count * 2; /* expect: prefer-const-declarations */
    final cast = count as int; /* expect: prefer-const-declarations */
    final invalidCast = count as String;
    print([doubled, cast, invalidCast]);
  }
}
