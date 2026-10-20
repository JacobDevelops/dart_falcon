enum Color { red, green, blue }

final shorthand = <Color, int>{.red: 1, .green: 2, .blue: 3};
const allColors = {Color.red: 1, Color.green: 2, Color.blue: 3};
final spread = <Color, int>{...allColors};
