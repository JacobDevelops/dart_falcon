import 'package:entity_base/entity_base.dart';
import 'support.dart';
class Mock {
  dynamic noSuchMethod(Invocation invocation) => null;
}
class InterfaceOnly extends Mock implements Parent {
  final int value = 1;
  List<Object?> get props => [];
}
class PackageComplete extends Entity {
  final int value = 1;
  List<Object?> get props => [value];
}
