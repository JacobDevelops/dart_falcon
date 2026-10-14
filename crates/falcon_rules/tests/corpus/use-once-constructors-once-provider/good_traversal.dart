class OnceProvider {
  const OnceProvider(Object? create);
  static OnceProvider once(Object? create) => new OnceProvider(create);
}

class Holder {
  final Object provider;
  const Holder(this.provider);
}

final explicitNew = new OnceProvider(null);
final explicitConst = const OnceProvider(null);
final nestedOnceFactories = [
  OnceProvider.once(OnceProvider.once(null)),
];

@Holder(OnceProvider(null))
class AnnotatedProvider {
  static const provider = OnceProvider(null);
  final runtimeProvider = OnceProvider.once(null);
}

final constList = const [OnceProvider(null)];
final constSet = const {OnceProvider(null)};
final constMap = const {'provider': OnceProvider(null)};
final constRecord = const (OnceProvider(null),);
final constConstruction = const Holder(OnceProvider(null));
const topLevelProvider = OnceProvider(null);

enum ProviderEnum {
  value(OnceProvider(null));

  final Object provider;
  const ProviderEnum(this.provider);
}

void constLocals([Object provider = const OnceProvider(null)]) {
  const localProvider = OnceProvider(null);
  print(localProvider);
}

void constPattern(Object value) {
  if (value case const (OnceProvider(null))) {
    print(value);
  }
}
