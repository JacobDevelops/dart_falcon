// legacyValue in comments and strings is not an identifier.
void example() {
  var modernValue = 'legacyValue';
  print(modernValue);
  var LegacyValue = 1;
}

class Derived {
  @override
  String toString() => 'Derived';
}
