function foo(x) {
    return x + 1;
}

var bar = function (x) {
    return x + 1;
};

if (foo(1) !== bar(1)) {
    return false;
}

var test3 = (function (x) { return x + 1; })(1);

if (test3 !== 2) {
    return false;
}

return true;
