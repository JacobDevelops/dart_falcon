class Throws {
  final Set<Type> types;
  const Throws([this.types = const {}]);
}

class DependencyFailure {}
@Throws({DependencyFailure})
void dependencyThrow() {}
class DependencyService {
  @Throws({DependencyFailure})
  DependencyService();
  @Throws({DependencyFailure})
  void run() {}
}
