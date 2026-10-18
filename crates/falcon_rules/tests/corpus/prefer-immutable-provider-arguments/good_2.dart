import 'package:riverpod/riverpod.dart';

final lists = ProviderFamily<int, List<int>>();
final sets = ProviderFamily<int, Set<int>>();
final maps = ProviderFamily<int, Map<String, int>>();
const topLevelList = <int>[1];
const topLevelSet = <int>{1};
const topLevelMap = <String, int>{'one': 1};

void constantArguments() {
  lists(const <int>[1]);
  sets(const <int>{1});
  maps(const <String, int>{'one': 1});
  lists(topLevelList);
  sets(topLevelSet);
  maps(topLevelMap);
  const localList = <int>[2];
  const localSet = <int>{2};
  const localMap = <String, int>{'two': 2};
  lists(localList);
  sets(localSet);
  maps(localMap);
}
