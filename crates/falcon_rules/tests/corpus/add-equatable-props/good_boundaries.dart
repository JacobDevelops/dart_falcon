import 'package:equatable/equatable.dart';
import 'package:entity_base/entity_base.dart';
import 'support.dart';
abstract class AbstractBase extends Equatable {}
sealed class SealedBase extends Equatable {}
class StoredProps extends Parent {
  @override
  final List<Object?> props = [];
}
class Mock {
  dynamic noSuchMethod(Invocation invocation) => null;
}
class MockState extends Mock implements Parent {}
class PackageComplete extends Entity {
  List<Object?> get props => [];
}
