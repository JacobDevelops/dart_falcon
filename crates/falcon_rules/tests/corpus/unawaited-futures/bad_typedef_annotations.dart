import 'package:meta/meta.dart' show awaitNotRequired;

@awaitNotRequired
typedef Task = Future<void> Function();
typedef Alias = Task;
@awaitNotRequired
typedef GenericTask<T> = Future<T> Function(T value);
typedef RequiredTask = Future<void> Function();

class Holder {
  Task field = () async {};
}

Future<void> typedefMetadata(Task task, Holder holder, RequiredTask required) async {
  task();
  task.call(); /* expect: unawaited-futures */
  holder.field();
  final alias = task;
  alias();
  final record = (callback: task,);
  record.callback();
  final list = <Task>[task];
  list.first();
  list[0]();
  required(); /* expect: unawaited-futures */
  final requiredAlias = required;
  requiredAlias(); /* expect: unawaited-futures */
}

Future<void> aliasMetadata(Alias task, GenericTask<int> generic, Task? nullable) async {
  task(); /* expect: unawaited-futures */
  generic(1);
  nullable!();
  nullable?.call(); /* expect: unawaited-futures */
  task.call(); /* expect: unawaited-futures */
  final structural = task as Future<void> Function();
  structural(); /* expect: unawaited-futures */
}

Future<void> joinedMetadata(
  bool flag,
  Task task,
  Task otherTask,
  RequiredTask required,
) async {
  (flag ? task : required)(); /* expect: unawaited-futures */
  (flag ? required : task)(); /* expect: unawaited-futures */
  (flag ? task : otherTask)(); /* expect: unawaited-futures */
  (flag ? task : task)();
  final copied = task;
  (flag ? task : copied)();
  final annotatedFirst = flag ? task : required;
  final requiredFirst = flag ? required : task;
  final sameTypedef = flag ? task : otherTask;
  annotatedFirst(); /* expect: unawaited-futures */
  requiredFirst(); /* expect: unawaited-futures */
  sameTypedef(); /* expect: unawaited-futures */
  [task, required][0](); /* expect: unawaited-futures */
  [required, task][0](); /* expect: unawaited-futures */
  [task, otherTask][0](); /* expect: unawaited-futures */
  [task, task][0]();
  [task, copied][0]();
  [task][0]();
  Task groupedA = task, groupedB = task;
  (flag ? groupedA : groupedB)();
  (flag ? [task] : [required])[0](); /* expect: unawaited-futures */
  (flag ? [required] : [task])[0]();
  (flag ? (task,) : (required,)).$1(); /* expect: unawaited-futures */
  (flag ? (required,) : (task,)).$1(); /* expect: unawaited-futures */
}

@awaitNotRequired
typedef ReturningTask<T> = Future<T> Function();

class TaskBox<T> {
  final ReturningTask<T> task;
  TaskBox(this.task);
}

Future<void> substitutedMetadata(
  bool flag,
  TaskBox<int> box,
  TaskBox<int> other,
) async {
  box.task();
  (flag ? box.task : box.task)(); /* expect: unawaited-futures */
  (flag ? box.task : other.task)(); /* expect: unawaited-futures */
  final copied = box.task;
  (flag ? copied : copied)();
  (flag ? box.task : copied)(); /* expect: unawaited-futures */
  final copiedAgain = box.task;
  (flag ? copied : copiedAgain)(); /* expect: unawaited-futures */
  [box.task, box.task][0](); /* expect: unawaited-futures */
  [box.task, other.task][0](); /* expect: unawaited-futures */
  [copied, copied][0]();
  [box.task, copied][0](); /* expect: unawaited-futures */
  [copied, copiedAgain][0](); /* expect: unawaited-futures */
  (flag ? [box.task] : [box.task])[0]();
  (flag ? [box.task] : [copied])[0]();
}
