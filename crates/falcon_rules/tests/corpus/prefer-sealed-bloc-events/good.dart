import 'package:bloc/bloc.dart';
import 'package:meta/meta.dart';

sealed class Action {}
final class First extends Action {}
final class Second extends Action {}
class Example extends Bloc<Action, int> { Example() : super(0); }
final class Other {}
class Another extends Bloc<Other, int> { Another() : super(0); }

class UnrelatedEvent {}
class UnrelatedState {}
class NoSuffix {}
