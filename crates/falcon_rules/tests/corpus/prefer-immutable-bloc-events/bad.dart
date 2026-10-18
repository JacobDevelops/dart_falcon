import 'package:bloc/bloc.dart';
import 'package:flutter_bloc/flutter_bloc.dart' as fb;
import 'package:meta/meta.dart';

abstract class Action {} /* expect: prefer-immutable-bloc-events */
class First extends Action {} /* expect: prefer-immutable-bloc-events */
class Second extends Action {} /* expect: prefer-immutable-bloc-events */
class Deep extends First {} /* expect: prefer-immutable-bloc-events */
class Other {} /* expect: prefer-immutable-bloc-events */
class Example extends Bloc<Action, int> { Example() : super(0); }
class Another extends fb.Bloc<Other, int> { Another() : super(0); }

class UnrelatedEvent {}
class UnrelatedState {}

@immutable
class Annotated extends Action {}
class MissingOwnAnnotation extends Annotated {} /* expect: prefer-immutable-bloc-events */
