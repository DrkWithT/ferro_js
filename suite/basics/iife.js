// test a JS IIFE mess

(function(x) {
    return (function (y) {
        return x + y;
    })(10);
})(10);
