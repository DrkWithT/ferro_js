// test < and >

var a = NaN, b = NaN, c = 1, d = 2, ok = 0;

if (a < b === undefined) {
    ok++;
}

if (a > b === undefined) {
    ok++;
}

if (c < d) {
    ok++;
}

if (d > c) {
    ok++;
}

// if ("AA" < "BB") {
//     ok++;
// }

ok === 4;
