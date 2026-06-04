// test prefix + operator

var a = 1, b = -1, c = "42", d = "test", ok = 0;

if (+a === 1) {
    ok++;
}

if (+b === -1) {
    ok++;
}

if (+c === 42) {
    ok++;
}

// TODO: add isNaN support!
// if (isNaN(+d)) {
//     ok++;
// }

return ok === 3;
