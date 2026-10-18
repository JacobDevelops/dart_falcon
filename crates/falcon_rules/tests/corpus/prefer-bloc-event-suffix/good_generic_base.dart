import 'package:bloc/bloc.dart';
class ActionEvent {}
class VariantEvent extends ActionEvent {}
abstract class Base<E, S> extends Bloc<E, S> { Base(S state) : super(state); }
class Example extends Base<ActionEvent, int> { Example() : super(0); }
