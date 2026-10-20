import 'package:riverpod/riverpod.dart';
import 'package:equatable/equatable.dart';

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

final records = ProviderFamily<int, dynamic>();
class EqualKey {
  final int id;
  EqualKey(this.id);
  bool operator ==(Object other) => other is EqualKey && other.id == id;
  int get hashCode => id.hashCode;
}

void acceptableFreshArguments() {
  records(const (<int>[1],));
  records((const <int>[1],));
  records((1, name: 'key'));
  records(EqualKey(1));
}

class InheritedKey extends EqualKey {
  InheritedKey(super.id);
}
class EquatableKey extends Equatable {
  final int id;
  EquatableKey(this.id);
  List<Object?> get props => [id];
}
class MixinKey with EquatableMixin {
  final int id;
  MixinKey(this.id);
  List<Object?> get props => [id];
}
class UnknownKey extends MissingBase {
  UnknownKey();
}
enum Status { ready }
void existingAndInheritedKeys(List<int> list, EqualKey key, void Function() callback) {
  records(EquatableKey(1));
  records(InheritedKey(1));
  records(MixinKey(1));
  records(UnknownKey());
  records(Status.ready);
  records(list);
  records(key);
  records(callback);
  final existing = <int>[1];
  records(existing);
  records((list, key: key));
}

class CachedKey {
  static final value = CachedKey();
  static CachedKey cached(int id) => value;
}
abstract class FactoryKey {
  factory FactoryKey(int id) = EqualFactoryKey;
}
class EqualFactoryKey implements FactoryKey {
  final int id;
  EqualFactoryKey(this.id);
  bool operator ==(Object other) => other is EqualFactoryKey && id == other.id;
  int get hashCode => id.hashCode;
}
extension type UserId(int id) {}
void nonGenerativeKeys() {
  records(CachedKey.cached(1));
  records(FactoryKey(1));
  records(UserId(1));
}

void acceptableWrappedArguments(bool flag, List<int> existing) {
  records(flag ? existing : const <int>[1]);
  records(const <int>[1] as List<int>);
  records((const <int>[1])!);
  records((flag ? existing : const <int>[1],));
  records(EqualKey(<int>[1].length));
}
