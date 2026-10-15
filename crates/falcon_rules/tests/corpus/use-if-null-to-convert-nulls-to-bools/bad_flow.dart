void checkedBool(bool? value, bool? replacement) {
  value == true; /* expect: use-if-null-to-convert-nulls-to-bools */
  if (value == null) return;
  value == true;
  {
    bool? value;
    value == true; /* expect: use-if-null-to-convert-nulls-to-bools */
  }
  value == true;
  value = replacement;
  value == true; /* expect: use-if-null-to-convert-nulls-to-bools */
}
void conditionalBool(bool? value) {
  if (value != null) value == true;
  value == true; /* expect: use-if-null-to-convert-nulls-to-bools */
  value != null && value == true;
}
