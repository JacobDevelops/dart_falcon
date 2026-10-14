final callback = () {
  final first = obj.property.value;
  final second = obj.property.value; /* expect: prefer-moving-to-variable */
};

void switchBody(int value) {
  switch (value) {
    case 0:
      final first = obj.property.value;
      final second = obj.property.value; /* expect: prefer-moving-to-variable */
      break;
  }
}

class DuplicateOperator {
  int operator +(int other) {
    final first = obj.property.value;
    final second = obj.property.value; /* expect: prefer-moving-to-variable */
    return first + second;
  }
}

void labeledBlock() {
  label: {
    final first = obj.property.value;
    final second = obj.property.value; /* expect: prefer-moving-to-variable */
  }
}
