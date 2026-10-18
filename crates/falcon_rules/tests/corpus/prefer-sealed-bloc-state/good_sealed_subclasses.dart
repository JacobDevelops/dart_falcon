import 'package:bloc/bloc.dart';
sealed class Action {}
sealed class Intermediate extends Action {}
final class Leaf extends Intermediate {}
class Example extends Cubit<Action> { Example() : super(Leaf()); }
