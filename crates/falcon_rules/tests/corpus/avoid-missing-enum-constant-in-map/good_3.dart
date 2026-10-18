enum Color { red, green, blue }

final empty = <Color, String>{};
Map<Color, int> contextual() => {};
final inactive = <Color, int>{if (false) Color.red: 1};
final nullableOnly = <Color?, int>{null: 1};
final fragments = <Color, int>{
  ...{Color.red: 1},
  Color.green: 2,
  Color.blue: 3,
};
Map<Color, int> conditionalFragments(bool flag) => <Color, int>{
  ...(flag ? {Color.red: 1} : {Color.red: 2}),
  Color.green: 2,
  Color.blue: 3,
};
final parenthesized = <Color, int>{
  ...?({Color.red: 1}),
  Color.green: 2,
  Color.blue: 3,
};
