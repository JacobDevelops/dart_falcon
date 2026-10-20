import 'colors.dart' as colors;

final partial = {colors.Color.red: 1}; /* expect: avoid-missing-enum-constant-in-map */
final complete = {colors.redAlias: 1, colors.Color.green: 2, colors.Color.blue: 3};
final empty = <colors.Color, int>{};

class Color {
  static const red = Color();
  static const blue = Color();
  const Color();
}
final unrelated = {Color.red: 1};
