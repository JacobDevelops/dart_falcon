class Surface {
  Surface(/** Parameter documentation. */ int value);
  void method({/** Named parameter documentation. */ int value = 0}) {}
}
void function(/** Parameter documentation. */ int value) {
  /** Local variable documentation. */
  int local = value;
  print(local);
}
