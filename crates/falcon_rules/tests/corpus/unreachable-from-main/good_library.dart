// A library without `main` or an entry-point pragma is not executable, so its
// unreferenced public API is not reported.

class PublicApi {
  void method() {}
}

void publicFunction() {}

int publicVariable = 0;

@pragma('vm:prefer-inline')
void inlined() {}
