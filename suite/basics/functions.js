var ok = 0;

function foo(x) {
    return x + 1;
}

var bar = function (x) {
    return x + 1;
};

var test3 = (function (x) { return x + 1; })(1);

if (foo(1) === bar(1)) {
    ok++;
}

if (test3 === 2) {
    ok++;
}

return ok === 2;
