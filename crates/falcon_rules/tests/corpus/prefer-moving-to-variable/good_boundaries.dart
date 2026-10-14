final callback = () {
  final first = obj.property.value;
  {
    final second = obj.property.value;
  }
};

void switchScopes(int value) {
  switch (value) {
    case 0:
      final first = obj.property.value;
      break;
    default:
      final second = obj.property.value;
      break;
  }
}
