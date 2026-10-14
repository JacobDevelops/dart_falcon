void ownDefault([bool value = true && false && true && false]) {}

void localDefault() {
  void nested([bool value = true && false && true && false]) {}
  final callback = ([bool value = true && false && true && false]) {};
}
