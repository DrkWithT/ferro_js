### TO DO's:

#### v0.1.0
 - Support more JS operators, including prefix, postfix, bitwise, and logical ops.
 - Support simple functions.
    - Support emitting simple function decls (NOTE: _Fail on returning closures, etc. until full object system is done!_)
 - Support simple objects with property semantics: data vs. accessor, writable + configurable + enumerable.
    - Generate bytecode for exotic / generic object literals. **WIP**
    - Generate bytecode for `get/set` accessors' usage. **WIP**
    - Test & Refactor:
      - Refactor properties to just be 3 JSValues & flags: `[[Value]], [[Get]], [[Set]], [[Flags]]`
      - Implement support for `foo.bar = 123` expressions.
 - Support assignment op results, conditional ternary op.
 - Add `typeof`, `delete`
   - TypeOf results:
      - Undefined: `"undefined"`
      - Null: `"object"`
      - Boolean: `"boolean"`
      - Number: `"number"`
      - String: `"string"`
      - ~~Symbol: `"symbol"`~~
      - non-Function Object: `"object"`
      - functions: `"function"`
   - Delete result:
      - The operation succeeds with `true` when property is non-accessor AND configurable.
 - Support strings.

#### v0.2.0
 - Support intrinsics for future:
   - Support generation of mappings to intrinsic objects.
   - Add pool for "immortal" and irremovable objects e.g `Date`, `Math`, `Boolean`, `Number`, `String`, `Array`, `Object`, and possibly `Promise`.
 - Support `Function` semantics e.g `.name, .length, .constructor, .call()`
 - Support native display(...args) function.
    - Trampoline to native calls with stub array that has `[PushUndef, NativeCall, Ret]`.
 - Add `break` and `continue` support.
 - Support more control flows: C-style `for`, `switch`
 - Handle computed property keys with caching them by string -> existing ID...
 - Support built-in `Date`, `Math`.
 - Support built-in `Array`, `Object`.
    - Support array (native object with array prototype)
    - Support array literals.
    - Support array prototype methods except `sort` and locales.
 - Support compound assignment ops: `*=`, `/=`, `+=`, `-=`
