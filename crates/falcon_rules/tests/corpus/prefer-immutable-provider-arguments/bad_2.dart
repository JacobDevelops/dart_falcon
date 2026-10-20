import 'package:riverpod/riverpod.dart';
import 'package:meta/meta.dart';

final lists = ProviderFamily<int, List<int>>();
final sets = ProviderFamily<int, Set<int>>();
final maps = ProviderFamily<int, Map<String, int>>();
const constantList = <int>[1];

void mutableArguments(List<int> list) {
  lists(<int>[1]); /* expect: prefer-immutable-provider-arguments */
  sets(<int>{1}); /* expect: prefer-immutable-provider-arguments */
  maps(<String, int>{'one': 1}); /* expect: prefer-immutable-provider-arguments */
  lists(list);
  final localList = <int>[2];
  lists(localList);
}

void shadowedConstant() {
  final constantList = <int>[3];
  lists(constantList);
}

@immutable
class Key {
  final int id;
  Key(this.id);
  Key.named(this.id);
}
typedef KeyAlias = Key;
class ChildKey extends Key {
  ChildKey(super.id);
}
final keys = ProviderFamily<int, Key>();
final callbacks = ProviderFamily<int, void Function()>();
void freshArguments() {
  keys(Key(1)); /* expect: prefer-immutable-provider-arguments */
  keys(Key.named(1)); /* expect: prefer-immutable-provider-arguments */
  keys(KeyAlias(1)); /* expect: prefer-immutable-provider-arguments */
  keys(ChildKey(1)); /* expect: prefer-immutable-provider-arguments */
  callbacks(() {}); /* expect: prefer-immutable-provider-arguments */
}

void nestedFreshArguments() {
  callbacks((() => 1)); /* expect: prefer-immutable-provider-arguments */
  keys(new Key(1)); /* expect: prefer-immutable-provider-arguments */
  callbacks((<int>[1],)); /* expect: prefer-immutable-provider-arguments */
  callbacks((key: Key(1),)); /* expect: prefer-immutable-provider-arguments */
}

void wrappedFreshArguments(bool flag, List<int> existing) {
  lists(flag ? <int>[1] : <int>[2]); /* expect: prefer-immutable-provider-arguments */
  lists(<int>[1] as List<int>); /* expect: prefer-immutable-provider-arguments */
  lists((<int>[1])!); /* expect: prefer-immutable-provider-arguments */
  lists(flag ? existing : <int>[2]); /* expect: prefer-immutable-provider-arguments */
}

void watchedFreshArguments(dynamic ref, bool flag) {
  ref.watch(lists(flag ? <int>[1] : <int>[2])); /* expect: prefer-immutable-provider-arguments */
}
