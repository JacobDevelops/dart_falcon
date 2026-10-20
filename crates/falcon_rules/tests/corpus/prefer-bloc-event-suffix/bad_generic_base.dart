import 'package:bloc/bloc.dart';
class Action {} /* expect: prefer-bloc-event-suffix */
class Variant extends Action {} /* expect: prefer-bloc-event-suffix */
abstract class Base<E, S> extends Bloc<E, S> { Base(S state) : super(state); }
abstract class Middle<E, S> extends Base<E, S> { Middle(S state) : super(state); }
class Example extends Middle<Action, int> { Example() : super(0); }
