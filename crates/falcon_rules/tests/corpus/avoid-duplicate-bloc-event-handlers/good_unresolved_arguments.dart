import 'package:bloc/bloc.dart';
import 'package:models/models.dart' show User, Post;

class Fetched<T> {}
class FetchBloc extends Bloc<Object, int> {
  FetchBloc() : super(0) {
    on<Fetched<User>>((event, emit) {});
    on<Fetched<Post>>((event, emit) {});
  }
}
