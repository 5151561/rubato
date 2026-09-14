//! Java 的数字 → 字符串形态。**全仓唯一一份**。
//!
//! 曾经有四份各写各的(gson / json-compat / xpath-compat / js_case_runner),
//! 其中 xpath-compat 那份漏了科学计数一支 —— `1e7` 给 `"10000000.0"` 而 Java 给
//! `"1.0E7"`,差分套盖不住(探针抽到的 `num()` 都是小数)。所以下沉到这里,
//! 谁要谁 `use`,单测钉在本文件。

/// 两个函数同一套写法,差的只有尾数的最短往返按哪种宽度取 ——
/// 宏比泛型省事:`f32`/`f64` 的 `is_nan` / `{:e}` 都是各自的固有方法,
/// 抽成泛型得先引一层数字 trait。
macro_rules! java_fp_to_string {
    ($v:ident) => {{
        if $v.is_nan() {
            return "NaN".into();
        }
        if $v.is_infinite() {
            return if $v > 0.0 { "Infinity".into() } else { "-Infinity".into() };
        }
        if $v == 0.0 {
            return if $v.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
        }
        let sci = format!("{:e}", $v); // 形如 "1.234e7" / "1e-5"
        let (mant, exp) = sci.split_once('e').expect("科学计数表示必有 e");
        let exp: i32 = exp.parse().expect("指数是整数");
        // exp 是 `1 <= |尾数| < 10` 时的十进制指数:`|v| < 1e-3` ⟺ exp < -3,
        // `|v| >= 1e7` ⟺ exp >= 7
        if (-3..7).contains(&exp) {
            let s = format!("{}", $v);
            if s.contains('.') { s } else { format!("{s}.0") }
        } else {
            let mant = if mant.contains('.') { mant.to_string() } else { format!("{mant}.0") };
            format!("{mant}E{exp}")
        }
    }};
}

/// `Double.toString(d)`。Kotlin 的 `Any.toString()`、gson 的数字序列化、
/// JsoupXpath 的 `XValue.asString()` 走的都是它。
///
/// 与 Rust 的 `{}` 有**两处**不同,两处都会咬人:
/// - 整数值也带 `.0`(`5` → `"5.0"`);
/// - 绝对值 `< 1e-3` 或 `>= 1e7` 转科学计数(`1e7` → `"1.0E7"`,
///   `1e-4` → `"1.0E-4"`)。
///
/// 尾数不自己算 —— 借 Rust 的最短往返指数形式(`{:e}`)再改写成 Java 的写法,
/// 自己乘除会引入舍入误差(`9007199254740993` 会变 `…992`)。
pub fn java_double_to_string(d: f64) -> String {
    java_fp_to_string!(d)
}

/// `Float.toString(f)`。与 [`java_double_to_string`] 同一套写法,只是尾数按
/// **float 的最短往返**取(`0.29f` 是 `"0.29"`,加宽成 f64 再打就成了
/// `0.28999999165534973`)。`FlexChildStyle` 那四个位是 `Float`,发现页分类
/// 的差分逐字比的就是这个形态。
///
/// JDK 19 起 `Float.toString` 也是最短往返(JDK-4511638),与 Rust 的 `{}`
/// 对齐;判据面的 toolchain 钉在 21,不必再管旧 JDK 那套。
pub fn java_float_to_string(f: f32) -> String {
    java_fp_to_string!(f)
}

#[cfg(test)]
mod tests {
    use super::java_double_to_string as s;
    use super::java_float_to_string as f;

    #[test]
    fn float_keeps_float_precision() {
        // 加宽到 f64 再打会变成 0.28999999165534973
        assert_eq!(f(0.29), "0.29");
        assert_eq!(f(-1.0), "-1.0");
        assert_eq!(f(0.0), "0.0");
        assert_eq!(f(1.0e8), "1.0E8");
    }

    #[test]
    fn integral_gets_dot_zero() {
        assert_eq!(s(5.0), "5.0");
        assert_eq!(s(-5.0), "-5.0");
        assert_eq!(s(0.0), "0.0");
        assert_eq!(s(-0.0), "-0.0");
    }

    #[test]
    fn decimal_form_inside_the_window() {
        assert_eq!(s(12.3), "12.3");
        assert_eq!(s(0.001), "0.001");
        assert_eq!(s(9999999.0), "9999999.0");
        assert_eq!(s(-0.001), "-0.001");
    }

    /// 这一支就是 xpath-compat 漏掉的那个洞
    #[test]
    fn scientific_form_outside_the_window() {
        assert_eq!(s(1e7), "1.0E7");
        assert_eq!(s(1.23e7), "1.23E7");
        assert_eq!(s(1e-4), "1.0E-4");
        assert_eq!(s(1e21), "1.0E21");
        assert_eq!(s(12345678.5), "1.23456785E7");
        assert_eq!(s(-1e7), "-1.0E7");
    }

    /// 尾数借 `{:e}` 而不是自己算 —— 这个数自己乘除会变成 …992
    #[test]
    fn mantissa_round_trips() {
        assert_eq!(s(9007199254740993.0), "9.007199254740992E15");
    }

    #[test]
    fn non_finite() {
        assert_eq!(s(f64::NAN), "NaN");
        assert_eq!(s(f64::INFINITY), "Infinity");
        assert_eq!(s(f64::NEG_INFINITY), "-Infinity");
    }
}
