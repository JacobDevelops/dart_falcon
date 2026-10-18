import 'package:bloc/bloc.dart';

class Marker { const Marker(); }
const immutable = Marker();

@immutable
class Action {} /* expect: prefer-immutable-bloc-events */
@immutable
class Child extends Action {} /* expect: prefer-immutable-bloc-events */
class Example extends Bloc<Action, int> { Example() : super(0); }
