import 'package:bloc/bloc.dart';

sealed class Action {}
class UnsealedChild extends Action {} /* expect: prefer-sealed-bloc-events */
class Example extends Bloc<Action, int> { Example() : super(0); }
