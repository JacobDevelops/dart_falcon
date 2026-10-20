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

final environmentBranch = <Color, int>{
  if (const bool.fromEnvironment('HAS_RED')) Color.red: 1 else Color.green: 1,
  Color.green: 2,
  Color.blue: 3,
};
const environmentFlag = bool.fromEnvironment('HAS_RED');
final nestedEnvironmentBranch = <Color, int>{
  ...{
    if (environmentFlag) Color.red: 1 else Color.green: 1,
  },
  Color.green: 2,
  Color.blue: 3,
};

const accent = bool.fromEnvironment('ACCENT') ? Color.red : Color.green;
final environmentKey = <Color, int>{accent: 1, Color.green: 2, Color.blue: 3};
final inlineEnvironmentKey = <Color, int>{
  (const bool.fromEnvironment('ACCENT') ? Color.red : Color.green): 1,
  Color.green: 2,
  Color.blue: 3,
};
const environmentTable = <Color, int>{
  if (bool.fromEnvironment('HAS_RED')) Color.red: 1,
  Color.green: 2,
  Color.blue: 3,
};
final namedEnvironmentSpread = <Color, int>{...environmentTable};
const conditionalEnvironmentTable = bool.fromEnvironment('HAS_RED')
    ? <Color, int>{Color.red: 1, Color.green: 2, Color.blue: 3}
    : <Color, int>{};
final conditionalEnvironmentSpread = <Color, int>{
  ...conditionalEnvironmentTable,
  Color.green: 2,
  Color.blue: 3,
};
void localEnvironmentCoverage() {
  const accent = bool.fromEnvironment('ACCENT') ? Color.red : Color.green;
  final map = <Color, int>{accent: 1, Color.green: 2, Color.blue: 3};
  const table = bool.fromEnvironment('HAS_RED')
      ? <Color, int>{Color.red: 1, Color.green: 2, Color.blue: 3}
      : <Color, int>{};
  final spread = <Color, int>{...table, Color.green: 2, Color.blue: 3};
}
const shadowedKey = Color.green;
void shadowedLocalKey() {
  const shadowedKey = Color.red;
  const key = shadowedKey;
  for (final shadowedKey in Color.values) {
    final map = <Color, int>{key: 1, Color.green: 2, Color.blue: 3};
  }
}
