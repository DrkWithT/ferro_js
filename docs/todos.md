### TO DO's:

#### v0.1.0
 - Support more JS operators, including prefix, postfix, bitwise, and logical ops.
 - Support simple functions.
    - Support emitting simple function decls (NOTE: _Fail on returning closures, etc. until full object system is done!_)
 - Support simple objects with property semantics: data vs. accessor, writable + configurable + enumerable.
    - Exotic / generic object literals.
    - `get/set` accessors as JSValues' each wrapping a Function.
 - Support `Function` semantics e.g `.name, .length, .constructor, .call()`
 - Support native display(...args) function.
    - Trampoline to native calls with stub array that has `[PushUndef, NativeCall, Ret]`.
 - Add `break` and `continue` support.
 - Support more control flows: C-style `for`, `switch`

#### v0.2.0
 - Add `typeof`, `delete`
 - Handle computed property keys with caching them by string -> existing ID...
 - Support built-in `Date`, `Math`.
 - Support built-in `Array`, `Object`.
    - Support array (native object with array prototype)
    - Support array literals.
    - Support array prototype methods except `sort` and locales.
 - Support compound assignment ops: `*=`, `/=`, `+=`, `-=`
