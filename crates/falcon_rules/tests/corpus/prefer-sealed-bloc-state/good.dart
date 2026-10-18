import 'package:bloc/bloc.dart';
import 'package:meta/meta.dart';

sealed class Snapshot {}
final class First extends Snapshot {}
final class Second extends Snapshot {}
class Example extends Bloc<int, Snapshot> { Example() : super(First()); }
final class Other {}
class Another extends Cubit<Other> { Another() : super(Other()); }

class UnrelatedEvent {}
class UnrelatedState {}
class NoSuffix {}
