// Negated filtered emptiness checks use any, not every.
bool negated(List<int> values) => !values.where((value) => value > 0).isEmpty;
bool parentheses(List<int> values) => !((values.where((value) => value > 0).isEmpty));
