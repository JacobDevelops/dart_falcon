import 'package:riverpod/riverpod.dart';

final lists = ProviderFamily<int, List<int>>();
final sets = ProviderFamily<int, Set<int>>();
final maps = ProviderFamily<int, Map<String, int>>();
const constantList = <int>[1];

void mutableArguments(List<int> list) {
  lists(<int>[1]); /* expect: prefer-immutable-provider-arguments */
  sets(<int>{1}); /* expect: prefer-immutable-provider-arguments */
  maps(<String, int>{'one': 1}); /* expect: prefer-immutable-provider-arguments */
  lists(list); /* expect: prefer-immutable-provider-arguments */
  final localList = <int>[2];
  lists(localList); /* expect: prefer-immutable-provider-arguments */
}

void shadowedConstant() {
  final constantList = <int>[3];
  lists(constantList); /* expect: prefer-immutable-provider-arguments */
}
