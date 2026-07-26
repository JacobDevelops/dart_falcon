String greeting(String name, String city) => 'Hello, $name from $city';
String adjacent = 'hello ' 'world';
String raw(String value) => r'raw' + value;

class CustomAdd {
  CustomAdd operator +(String value) => this;
}
CustomAdd custom(CustomAdd value) => value + 'text';
