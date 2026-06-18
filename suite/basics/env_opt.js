var a = 1, ok = 0;

// 1: no capturing env
function f1(x) {
    return x;
}

// 2: capturing global env
function f2() {
    return a;
}

// 4: nested function captures
function f4() {
    function t2() {
        return a;
    }

    return t2();
}

if (f1(2) === 2) {
    ok++;
}

if (f2() === 1) {
    ok++;
}

if (f4() === 1) {
    ok++;
}

return ok === 3;
