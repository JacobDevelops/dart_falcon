// Declaration-only SDK excerpt for SDK-free semantic tests and corpus tools.
library dart.collection;
import 'dart:core';
abstract class ListBase<E> implements List<E> {}
abstract class UnmodifiableListView<E> extends ListBase<E> {}
