class BaseOwner {
  @deprecated
  String toString() => '';
  void member() {}
}
mixin AppliedOwner on BaseOwner {
  @deprecated
  void member() {}
}
class ConcreteOwner extends BaseOwner with AppliedOwner {}
mixin ConstrainedOwner on ConcreteOwner {
  void uses() {
    super.toString(); /* expect: deprecated-member-use-from-same-package */
    super.member(); /* expect: deprecated-member-use-from-same-package */
  }
}

class ElementBase {}
class IntElement extends ElementBase {
  @deprecated
  void selectedElement() {}
}
class StringElement extends ElementBase {}
class SourceIterable<T> {}
class SourceList<T> extends SourceIterable<T> {}
class IntSourceList extends SourceList<IntElement> {}
extension StringListOwner on SourceList<StringElement> {
  @deprecated
  void genericMember() {}
}
extension ObjectListOwner on Object {
  void genericMember() {}
}
void genericUses(SourceList<IntElement> ints, SourceList<StringElement> strings,
    IntSourceList inherited) {
  ints.genericMember();
  inherited.genericMember();
  strings.genericMember(); /* expect: deprecated-member-use-from-same-package */
}

class SourceMap<K, V> {}
extension RawListOwner on SourceList {
  @deprecated
  void rawMember() {}
}
extension RawIterableOwner on SourceIterable {
  void rawMember() {}
}
extension GenericListOwner<T> on SourceList<T> {
  @deprecated
  void inferredMember() {}
  @deprecated
  void concreteIterableMember() {}
}
extension GenericIterableOwner<U> on SourceIterable<U> {
  void inferredMember() {}
}
extension ConcreteIterableOwner on SourceIterable<IntElement> {
  void concreteIterableMember() {}
}
extension TextMapOwner<K> on SourceMap<K, StringElement> {
  @deprecated
  void mapMember() {}
}
extension BoundedListOwner<T extends IntElement> on SourceList<T> {
  @deprecated
  void boundedMember() {}
}
extension GeneralGenericOwner on Object {
  void mapMember() {}
  void boundedMember() {}
}
void inferredOwnerUses(
    SourceList<IntElement> ints,
    SourceList<StringElement> strings,
    SourceList raw,
    SourceMap<IntElement, IntElement> intMap,
    SourceMap<IntElement, StringElement> textMap) {
  ints.rawMember(); /* expect: deprecated-member-use-from-same-package */
  raw.rawMember(); /* expect: deprecated-member-use-from-same-package */
  ints.inferredMember(); /* expect: deprecated-member-use-from-same-package */
  ints.concreteIterableMember(); /* expect: deprecated-member-use-from-same-package */
  ints.boundedMember(); /* expect: deprecated-member-use-from-same-package */
  strings.boundedMember();
  intMap.mapMember();
  textMap.mapMember(); /* expect: deprecated-member-use-from-same-package */
}

class PlainConstraint {}
class ImplementedOwner {
  @deprecated
  String toString() => '';
}
mixin ImplementingOwner on PlainConstraint implements ImplementedOwner {
  String describe() => super.toString();
}

extension ExactListOwner on SourceList<IntElement> {
  @deprecated
  void exactMember() {}
}
extension AnyListOwner<T> on SourceList<T> {
  void exactMember() {}
  T pick() => throw 0;
}
void tiedOwnerUses(SourceList<IntElement> ints) {
  ints.exactMember(); /* expect: deprecated-member-use-from-same-package */
}
void boundedReceiverUses<X extends SourceList<IntElement>>(X values) {
  values.pick().selectedElement(); /* expect: deprecated-member-use-from-same-package */
}

class SourceFuture<T> {}
extension FutureGetterOwner<T> on SourceFuture<T> {
  @deprecated
  void get completed {}
}
void futureGetterUses(SourceFuture<void> future, SourceFuture<void>? nullable) {
  future.completed; /* expect: deprecated-member-use-from-same-package */
  nullable?.completed; /* expect: deprecated-member-use-from-same-package */
}

extension FunctionValueOwner<T> on T {
  @deprecated
  void invokeCallback() {}
}
void functionReceiverUses(void Function() callback) {
  callback.invokeCallback(); /* expect: deprecated-member-use-from-same-package */
}
