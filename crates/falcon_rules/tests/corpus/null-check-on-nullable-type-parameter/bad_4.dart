// Direct keys retain their map key context, including trivia and grouped operands.
void directKey<T>(T? value) {
  final Map<T, int> result = {value!: 0}; /* expect: null-check-on-nullable-type-parameter */
  print(result);
}
void commentKey<T>(T? value) {
  final Map<T, int> result = { /* (comment) */ value!: 0}; /* expect: null-check-on-nullable-type-parameter */
  print(result);
}
void groupedOperand<T>(T? value) {
  final Map<T, int> result = {(value)!: 0}; /* expect: null-check-on-nullable-type-parameter */
  print(result);
}
void nullAwareKey<T>(T? value) {
  final Map<T, int> result = {? /* (comment) */ value!: 0}; /* expect: null-check-on-nullable-type-parameter */
  print(result);
}
void conditionalKey<T>(T? value, bool flag) {
  final Map<T, int> result = {if (flag) /* (comment) */ value!: 0}; /* expect: null-check-on-nullable-type-parameter */
  print(result);
}
// Grouped assertions use the map value context, which can also be T.
void sameTypeKey<T>(T? value, T other) {
  final Map<T, T> result = {(value!): other}; /* expect: null-check-on-nullable-type-parameter */
  print(result);
}
