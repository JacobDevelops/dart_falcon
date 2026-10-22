enum Color { red, green, blue }
const alias = Color.red;
typedef Shade = Color;

final incomplete = {Color.red: 'red'}; /* expect: avoid-missing-enum-constant-in-map, msg: "Include every `Color` value in this map; missing: `green`, `blue`." */
final aliases = {alias: 1, Color.green: 2}; /* expect: avoid-missing-enum-constant-in-map */
final typedefKey = <Shade, int>{Color.blue: 3}; /* expect: avoid-missing-enum-constant-in-map */
Map<Color, int> contextual() => {Color.green: 2}; /* expect: avoid-missing-enum-constant-in-map */
final conditional = <Color, int>{if (true) Color.red: 1}; /* expect: avoid-missing-enum-constant-in-map */

void local() {
  const red = Color.red;
  final map = {red: 1, Color.blue: 3}; /* expect: avoid-missing-enum-constant-in-map */
}
