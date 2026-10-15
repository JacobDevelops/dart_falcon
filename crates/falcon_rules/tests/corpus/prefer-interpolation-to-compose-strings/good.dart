String greeting(String name, String city) => 'Hello, $name from $city';
String adjacent = 'hello ' 'world';
String raw(String value) => r'raw' + value;

class CustomAdd {
  CustomAdd operator +(String value) => this;
}
CustomAdd custom(CustomAdd value) => value + 'text';

// Two literals, interpolated or not, are not a composition.
String interpolated(String name) => 'Hello $name' + '!';

class Padded {
  @override
  String toString({int pad = 0}) => '';
}
String padded(Padded value) => 'x' + value.toString(pad: 1);
