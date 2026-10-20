void listen({required void Function() onTap}) {}
void consume(void Function() callback) {}
class Controller {
  void tap() {} /* expect: prefer-correct-handler-name */
  void work() {} /* expect: prefer-correct-handler-name */
  void positionalWork() {} /* expect: prefer-correct-handler-name */
  void install() {
    listen(onTap: this.tap);
    listen(onTap: this.tap);
    listen(onTap: work);
    consume(positionalWork);
  }
}
