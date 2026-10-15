// @dart = 2.14

class Constructor {
  Constructor(int value);
}

void oldLanguage(List<int> values) {
  values.map((value) => Constructor(value));
  values.map((value) => new Constructor(value));
}
