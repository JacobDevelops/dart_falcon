void listen({required void Function() onTap}) {}
void consume(void Function() callback) {}
void globalWork() {}
class Controller {
  void _handleTap() {}
  void ordinaryMethod() {}
  void install(void Function() ordinaryMethod) {
    listen(onTap: this._handleTap);
    consume(this._handleTap);
    listen(onTap: () {});
    listen(onTap: ordinaryMethod);
    consume(globalWork);
  }
}
class Child extends Controller {
  void ordinaryMethod() {}
  void wire() { listen(onTap: this.ordinaryMethod); }
}
abstract class HandlerContract {
  void tap();
}
class HandlerImplementation implements HandlerContract {
  void tap() {}
  void wire() { listen(onTap: tap); }
}
