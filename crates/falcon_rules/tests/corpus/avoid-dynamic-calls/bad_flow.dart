void dynamicBranch(dynamic value, dynamic replacement) {
  value.length; /* expect: avoid-dynamic-calls */
  if (value is String) {
    value.length;
    value = replacement;
    value.length; /* expect: avoid-dynamic-calls */
  } else {
    value.length; /* expect: avoid-dynamic-calls */
  }
  value.length; /* expect: avoid-dynamic-calls */
}
void dynamicReturn(dynamic value) {
  if (value is! String) return;
  value.length;
  {
    dynamic value;
    value.length; /* expect: avoid-dynamic-calls */
  }
  value.length;
}
void dynamicCast(dynamic value, dynamic replacement) {
  value as String;
  value.length;
  value = replacement;
  value.length; /* expect: avoid-dynamic-calls */
}
