class GenericDefaults<T> {
  List<T> toList({bool growable = true}) => <T>[];
  void independent({int value = 0}) {}
}
void substitutedDefaults(GenericDefaults<int> values) {
  values.toList(growable: true);
  values.independent(value: 0); /* expect: avoid-redundant-argument-values */
}
