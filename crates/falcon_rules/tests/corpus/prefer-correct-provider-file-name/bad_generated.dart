import 'package:riverpod_annotation/riverpod_annotation.dart';

part 'bad_generated.g.dart';

@riverpod
int counter(Ref ref) => 0; /* expect: prefer-correct-provider-file-name */
