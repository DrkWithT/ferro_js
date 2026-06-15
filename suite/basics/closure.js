function makeTest1() {
    var a = 67;

    function foo() {
        return a;
    }

    return foo;
}

var f = makeTest1();

f() === 67;
