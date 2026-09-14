// 两侧共享的**确定性垫片**。js_diff.sh 把本文件复制到工作目录,裁判(jsharness)
// 与被测(js_case_runner)各自读同一份文本 —— 保证逐字一致,不靠两边手抄。
//
// 为什么要它:书源里大量写 `Math.round(new Date()/1000)` 然后把时间戳拼进
// 待签名串,再 `java.md5Encode(...)`。两侧不可能在同一毫秒跑,于是时间戳与
// 由它派生的签名必然不同 —— 那不是实现差异,是差分自身的噪声(实测 803 例里
// 有 17 例每次运行都在变)。冻住时钟后这些用例才真正在比「算法一致」。
//
// **完成值必须不受影响**:裁判侧是把本文件**前置拼到用例代码前**再求值的,
// 所以整段只能是一条 `var` 语句 —— VariableStatement 的完成值是空,
// 拼接后整段脚本的完成值仍旧来自用例自己那段。别改成 IIFE 表达式语句。
//
// 冻的两样东西(书源可观察的全部非确定性来源):
//   1. 时钟:`Date.now()` / `new Date()`(无参)→ 固定纪元;
//   2. `Math.random()` → 固定种子的 LCG(两侧同一段 JS,故同一串数)。
// 宿主侧的非确定性(`java.randomUUID` / `androidId`)不在这里,
// 走 README「不可差分面」的固定常量。
var __rubatoDeterminism = (function () {
    // 2026-08-29T12:00:00Z —— 取值本身无所谓,重要的是两侧同一个
    var FIXED_EPOCH = 1788001200000;

    var RealDate = Date;
    function FrozenDate(a, b, c, d, e, f, g) {
        switch (arguments.length) {
            case 0: return new RealDate(FIXED_EPOCH);
            case 1: return new RealDate(a);
            case 2: return new RealDate(a, b);
            case 3: return new RealDate(a, b, c);
            case 4: return new RealDate(a, b, c, d);
            case 5: return new RealDate(a, b, c, d, e);
            case 6: return new RealDate(a, b, c, d, e, f);
            default: return new RealDate(a, b, c, d, e, f, g);
        }
    }
    // 构造函数返回对象时 `new` 取该对象,故 `new FrozenDate()` 与
    // `FrozenDate()` 都给一个真 Date;原型照搬,`instanceof` 不变
    FrozenDate.prototype = RealDate.prototype;
    FrozenDate.now = function () { return FIXED_EPOCH; };
    FrozenDate.parse = RealDate.parse;
    FrozenDate.UTC = RealDate.UTC;
    Date = FrozenDate;

    // LCG(数值配方同 java.util.Random 的常数,取高 32 位),两侧逐位一致
    var seed = 20260829;
    Math.random = function () {
        seed = (seed * 1103515245 + 12345) % 2147483648;
        return seed / 2147483648;
    };

    return FIXED_EPOCH;
})();
