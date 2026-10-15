// The analyzer reports the final nullable comparison, but not the promoted const.
void finalNullableBooleans() {
  final bool? flag = true;
  flag == true; /* expect: use-if-null-to-convert-nulls-to-bools */
  const bool? enabled = true;
  enabled == true;
  bool? mutable = true;
  mutable == true;
  final inferred = true;
  inferred == true;
}
