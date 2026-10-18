import 'package:bloc/bloc.dart';
import 'package:flutter_bloc/flutter_bloc.dart' as fb;
import 'package:meta/meta.dart';

abstract class Action {} /* expect: prefer-sealed-bloc-events */
class First extends Action {} /* expect: prefer-sealed-bloc-events */
class Second extends Action {} /* expect: prefer-sealed-bloc-events */
class Deep extends First {} /* expect: prefer-sealed-bloc-events */
class Other {} /* expect: prefer-sealed-bloc-events */
class Example extends Bloc<Action, int> { Example() : super(0); }
class Another extends fb.Bloc<Other, int> { Another() : super(0); }

class UnrelatedEvent {}
class UnrelatedState {}
