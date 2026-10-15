void inheritedSdkChain(List<int> values) {
  values.where((value) => true).firstWhere((value) => true, orElse: null); /* expect: null-closures */
}
