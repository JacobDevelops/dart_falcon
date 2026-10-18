import 'package:bloc/bloc.dart';

class Marker { const Marker(); }
const immutable = Marker();

@immutable
class Action {} /* expect: prefer-immutable-bloc-state */
@immutable
class Child extends Action {} /* expect: prefer-immutable-bloc-state */
class Example extends Cubit<Action> { Example() : super(Action()); }
