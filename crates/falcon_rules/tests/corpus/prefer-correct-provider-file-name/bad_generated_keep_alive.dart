import 'package:riverpod_annotation/riverpod_annotation.dart' as annotation;

part 'bad_generated_keep_alive.g.dart';

@annotation.Riverpod(keepAlive: true)
int counter(annotation.Ref ref) => 0; /* expect: prefer-correct-provider-file-name */
