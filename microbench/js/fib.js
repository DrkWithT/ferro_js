// recursive fibonacci

function fib(n) {
    if (n === 0) {
        return n;
    } else if (n === 1) {
        return n;
    } else {
        return fib(n - 1) + fib(n - 2);
    }
}

fib(30);
