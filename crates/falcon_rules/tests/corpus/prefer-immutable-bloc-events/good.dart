import 'package:bloc/bloc.dart';
import 'package:meta/meta.dart';

@immutable
sealed class Action {}
@immutable
final class First extends Action {}
@immutable
final class Second extends Action {}
class Example extends Bloc<Action, int> { Example() : super(0); }
@immutable
final class Other {}
class Another extends Bloc<Other, int> { Another() : super(0); }

class UnrelatedEvent {}
class UnrelatedState {}
class NoSuffix {}
