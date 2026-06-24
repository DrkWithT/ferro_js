### TO DO's:

#### v0.1.0
 - Support more JS operators, including prefix, postfix, bitwise, and logical ops.
 - Support simple functions.
 - Support simple objects with property semantics: data vs. accessor.
 - Implement closures.
 - Support relational ops, bitshifts, conditional ternary op, assignment op, comma.
 - Support primitive strings.
 - Add `typeof`:
   - TypeOf results:
      - Undefined: `"undefined"`
      - Null: `"object"`
      - Boolean: `"boolean"`
      - Number: `"number"`
      - String: `"string"`
      - ~~Symbol: `"symbol"`~~
      - non-Function Object: `"object"`
      - functions: `"function"`
 - Refactor codebase into modular, Dep. Injected driver.
 - Support native console.log(...args) function.
    - Trampoline to native calls with stub array that has `[PushUndef, NativeCall, Ret]`.
 - Add `break` and `continue` support.
 - Add GC for objects and dead strings.
   - Add tenuring logic pre-run for emitter.

#### v0.2.0
 - Support intrinsics for future:
   - Support generation of mappings to intrinsic objects.
   - Add pool for "immortal" and irremovable objects e.g `Date`, `Math`, `Boolean`, `Number`, `String`, `Array`, `Object`, and possibly `Promise`.
 - Support built-in `Array`, `Object`.
    - Support `Object.create`, `Object.freeze()`, `Object.seal()`, etc.
    - Support arrays (exotic object with array prototype)
    - Support array literals.
    - Support array prototype methods except `sort` and locales.
 - Add `delete`:
      - The operation succeeds with `true` when property is non-accessor AND configurable AND not a direct name. Otherwise, a `TypeError` occurs in `'use strict'` mode when `false`. Non properties return `true` vacuously.
      - NOTE: writable + configurable + enumerable are 3 important flags per property to check at runtime.
         - Set: Not `configurable` and `writable` names will "delete" with `false` in loose mode.
         - Delete: Not `configurable` and `accessor` based names will "delete" with `false`.
         - Maybe Shapes should store this metadata.
 - Support more control flows: C-style `for`, `switch`
 - Support compound assignment ops: `*=`, `/=`, `+=`, `-=`
 - Handle computed property keys with caching them by string -> existing ID...
 - Support built-in `Date`, `Math`.
 - Support `Function` semantics e.g `.name, .length, .constructor, .call()`
