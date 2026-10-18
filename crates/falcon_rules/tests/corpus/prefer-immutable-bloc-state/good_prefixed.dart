import 'package:bloc/bloc.dart' as bloc;
import 'package:meta/meta.dart' as meta;

@meta.immutable
class Action {}
@meta.immutable
class Child extends Action {}
class Example extends bloc.Cubit<Action> { Example() : super(Action()); }
