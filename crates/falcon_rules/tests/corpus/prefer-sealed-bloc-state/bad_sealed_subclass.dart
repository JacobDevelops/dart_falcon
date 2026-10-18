import 'package:bloc/bloc.dart';

sealed class Action {}
class UnsealedChild extends Action {} /* expect: prefer-sealed-bloc-state */
class Example extends Cubit<Action> { Example() : super(UnsealedChild()); }
