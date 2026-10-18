enum Color { red, green, blue }
enum Other { red, green, blue }
const redAlias = Color.red;

final complete = {Color.red: 1, Color.green: 2, Color.blue: 3};
final aliases = {redAlias: 1, Color.green: 2, Color.blue: 3};
final strings = {'red': 1};
final integers = <int, String>{};
final unrelated = <Object, int>{Color.red: 1, Other.green: 2};
final nullable = <Color?, int>{null: 0, Color.red: 1, Color.green: 2, Color.blue: 3};

Map<Color, int> unknownKey(Color color) => {color: 1};
Map<Color, int> unknownSpread(Map<Color, int> source) => {...source};
Map<Color, int> dynamicBranch(bool flag) => {
  Color.red: 1,
  if (flag) Color.green: 2 else Color.blue: 3,
};
Map<Color, int> forValues() => {for (final color in Color.values) color: 1};

class NotAnEnum {
  static const first = NotAnEnum();
  static const second = NotAnEnum();
  const NotAnEnum();
}
final notEnum = {NotAnEnum.first: 1};
