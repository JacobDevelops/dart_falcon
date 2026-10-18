import 'package:bloc/bloc.dart';
import 'package:flutter_bloc/flutter_bloc.dart' as fb;
import 'package:meta/meta.dart';

abstract class Snapshot {} /* expect: prefer-immutable-bloc-state */
class First extends Snapshot {} /* expect: prefer-immutable-bloc-state */
class Second extends Snapshot {} /* expect: prefer-immutable-bloc-state */
class Deep extends First {} /* expect: prefer-immutable-bloc-state */
class Other {} /* expect: prefer-immutable-bloc-state */
class Example extends Bloc<int, Snapshot> { Example() : super(First()); }
class Another extends fb.Cubit<Other> { Another() : super(Other()); }

class UnrelatedEvent {}
class UnrelatedState {}

@immutable
class Annotated extends Snapshot {}
class MissingOwnAnnotation extends Annotated {} /* expect: prefer-immutable-bloc-state */
