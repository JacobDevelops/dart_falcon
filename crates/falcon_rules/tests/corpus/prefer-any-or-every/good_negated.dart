// Already handled by prefer-iterable-every.
bool negated(List<int> values) => !values.where((value) => value > 0).isEmpty;
bool parentheses(List<int> values) => !((values.where((value) => value > 0).isEmpty));
