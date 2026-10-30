import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_bloc/flutter_bloc.dart' as fb;

typedef BuilderAlias = fb.BlocBuilder<Object, int>;

void examples() {
  BlocBuilder(builder: (context, state) => state); /* expect: avoid-empty-build-when */
  new BlocBuilder(builder: (context, state) => state); /* expect: avoid-empty-build-when */
  const BlocBuilder(); /* expect: avoid-empty-build-when */
  BlocBuilder<Object, int>(builder: (context, state) => state); /* expect: avoid-empty-build-when */
  fb.BlocBuilder<Object, int>(); /* expect: avoid-empty-build-when */
  BlocConsumer(listener: (context, state) {}, builder: (context, state) => state); /* expect: avoid-empty-build-when */
  new fb.BlocConsumer<Object, int>(); /* expect: avoid-empty-build-when */
  const fb.BlocConsumer<Object, int>(); /* expect: avoid-empty-build-when */
  BuilderAlias(); /* expect: avoid-empty-build-when */
}
