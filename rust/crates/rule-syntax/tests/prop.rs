//! 属性测试:
//! 1. 任意输入不 panic(只允许 Ok/Err);
//! 2. 不含筛选器/引号/转义字符的输入,splitRule 等价于朴素字符串切分。

use proptest::prelude::*;
use rule_syntax::RuleAnalyzer;

proptest! {
    #[test]
    fn never_panics(data in "[a-z@&|%\\[\\]()'\"\\\\{}$.。中文🎉 ]{0,64}") {
        let _ = RuleAnalyzer::new(&data, false).split_rule(&["&&", "||", "%%"]);
        let _ = RuleAnalyzer::new(&data, true).split_rule(&["&&", "||"]);
        let _ = RuleAnalyzer::new(&data, false).split_rule(&["@"]);
        let mut ra = RuleAnalyzer::new(&data, false);
        let _ = ra.trim();
        let _ = ra.split_rule(&["@"]);
        let _ = RuleAnalyzer::new(&data, false)
            .inner_rule("{$.", 1, 1, |s| Some(format!("<{s}>")));
        let _ = RuleAnalyzer::new(&data, false)
            .inner_rule_str("{{", "}}", |s| Some(format!("<{s}>")));
    }

    #[test]
    fn plain_input_equals_naive_split(data in "[a-z@&|%中文.]{0,64}") {
        // 输入不含 [ ( ' " \,切分应与朴素 split 一致
        let got = RuleAnalyzer::new(&data, false)
            .split_rule(&["&&", "||", "%%"])
            .unwrap();
        // 朴素模型:elementsType = 最早命中的分隔符(同位置按声明序),
        // 之后整串只按该分隔符切分
        let mut best: Option<(usize, &str)> = None;
        for sep in ["&&", "||", "%%"] {
            if let Some(p) = data.find(sep) {
                if best.is_none_or(|(bp, _)| p < bp) {
                    best = Some((p, sep));
                }
            }
        }
        let expect: Vec<String> = match best {
            None => vec![data.clone()],
            Some((_, sep)) => data.split(sep).map(String::from).collect(),
        };
        prop_assert_eq!(got, expect);
    }
}
