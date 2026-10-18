import 'package:equatable/equatable.dart';
class Parent extends Equatable {
  final int parent = 0;
  @override
  List<Object?> get props => [parent];
}
class Middle extends Parent {}
class Mixed with EquatableMixin {
  @override
  List<Object?> get props => [];
}
