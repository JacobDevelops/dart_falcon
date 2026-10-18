import 'package:bloc/bloc.dart';
class Action {} /* expect: prefer-bloc-state-suffix */
class Variant extends Action {} /* expect: prefer-bloc-state-suffix */
abstract class Base<E, S> extends Bloc<E, S> { Base(S state) : super(state); }
abstract class Middle<E, S> extends Base<E, S> { Middle(S state) : super(state); }
class Example extends Middle<int, Action> { Example() : super(Action()); }
