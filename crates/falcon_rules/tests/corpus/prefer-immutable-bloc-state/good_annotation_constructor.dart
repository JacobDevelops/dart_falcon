import 'package:bloc/bloc.dart';
import 'package:meta/meta.dart' as meta;

@meta.Immutable('model')
class Action {}
@meta.Immutable('child')
class Child extends Action {}
class Example extends Cubit<Action> { Example() : super(Action()); }
