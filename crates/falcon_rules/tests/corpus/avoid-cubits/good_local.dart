import 'package:bloc/bloc.dart' as b;

class Cubit<T> {}
class Local extends Cubit<int> {}
class IndirectLocal extends Local {}
class Bloc<T> {}
class LocalBloc extends Bloc<int> {}
class Unrelated {}
b.Cubit<int>? reference;
