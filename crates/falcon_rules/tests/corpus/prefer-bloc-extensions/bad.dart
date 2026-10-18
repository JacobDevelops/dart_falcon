import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_bloc/flutter_bloc.dart' as fb;
void example(Object context) {
  BlocProvider.of<int>(context); /* expect: prefer-bloc-extensions */
  BlocProvider.of<int>(context, listen: false); /* expect: prefer-bloc-extensions */
  BlocProvider.of<int>(context, listen: true); /* expect: prefer-bloc-extensions */
  fb.BlocProvider.of<int>(context); /* expect: prefer-bloc-extensions */
  RepositoryProvider.of<int>(context); /* expect: prefer-bloc-extensions */
}
