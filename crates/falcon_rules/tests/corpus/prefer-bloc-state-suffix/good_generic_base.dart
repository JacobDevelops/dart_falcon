import 'package:bloc/bloc.dart';
class ActionState {}
class VariantState extends ActionState {}
abstract class Base<E, S> extends Bloc<E, S> { Base(S state) : super(state); }
class Example extends Base<int, ActionState> { Example() : super(ActionState()); }
