function makeTest1() {
    var a = 67;

    return (function() {
        return a;
    });
}

var f = makeTest1();

return f() === 67;
