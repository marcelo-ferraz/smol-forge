use std::cmp::max;

use heck::{
    ToKebabCase, ToLowerCamelCase, ToPascalCase, ToShoutyKebabCase, ToShoutySnakeCase, ToSnakeCase,
    ToTitleCase, ToTrainCase,
};

use crate::models::{Modifier, Output, RenderType, WeldToken};

pub fn build_string(parts: Vec<WeldToken>, render_type: &RenderType) -> String {
    let mut result = String::new();
    for part in parts {
        let partial = build_from_token(part);
        result.push_str(&partial);
    }

    // remove r# from identifiers but keep as is for literals
    if let RenderType::Identifier = render_type {
        return result.replace("r#", "");
    }

    result
}

fn build_from_token(part: WeldToken) -> String {
    match part {
        WeldToken::Plain(value) => value.clone(),
        WeldToken::ConcatGroup(grouped_parts) => grouped_parts
            .iter()
            .map(|sub_part| build_from_token(sub_part.clone()))
            .collect::<String>(),
        WeldToken::ListGroup(grouped_parts) => grouped_parts
            .iter()
            .map(|sub_part| build_from_token(sub_part.clone()))
            .collect::<String>(),
        WeldToken::Modify(target, modifiers) => match *target {
            WeldToken::Plain(value) => modify_single(value, &modifiers),
            WeldToken::ListGroup(items) => {
                let value = items
                    .iter()
                    .map(|sub_part| build_from_token(sub_part.clone()))
                    .collect::<Vec<String>>();

                modify_list(value, &modifiers)
            }
            WeldToken::ConcatGroup(grouped_parts) => {
                let value = grouped_parts
                    .iter()
                    .map(|sub_part| build_from_token(sub_part.clone()))
                    .collect::<String>();

                modify_single(value, &modifiers)
            }
            WeldToken::Modify(token_part, nested_modifiers) => {
                let nested_result = modify_single(build_from_token(*token_part), &nested_modifiers);

                modify_single(nested_result, &modifiers)
            }
        },
    }
}

fn modify_single(value: String, modifiers: &Vec<Modifier>) -> String {
    let values = vec![value.to_string()];

    modify_list(values, modifiers)
}

fn each(values: &mut [String], f: impl Fn(&str) -> String) {
    values.iter_mut().for_each(|val| {
        *val = f(val);
    });
}

fn modify_list(mut values: Vec<String>, modifiers: &Vec<Modifier>) -> String {
    for modified in modifiers {
        let list_mode = values.len() > 1;

        match modified {
            Modifier::Singular => {
                values
                    .iter_mut()
                    .filter(|val| val.ends_with('s'))
                    .for_each(|val| {
                        val.pop();
                    });
            }
            Modifier::Plural => {
                values
                    .iter_mut()
                    .filter(|val| !val.ends_with('s'))
                    .for_each(|val| {
                        val.push('s');
                    });
            }
            Modifier::Lowercase => each(&mut values, str::to_lowercase),
            Modifier::Uppercase => each(&mut values, str::to_uppercase),
            Modifier::PascalCase => each(&mut values, str::to_pascal_case),
            Modifier::LowerCamelCase => each(&mut values, str::to_lower_camel_case),
            Modifier::SnakeCase => each(&mut values, str::to_snake_case),
            Modifier::KebabCase => each(&mut values, str::to_kebab_case),
            Modifier::ShoutySnakeCase => each(&mut values, str::to_shouty_snake_case),
            Modifier::TitleCase => each(&mut values, str::to_title_case),
            Modifier::ShoutyKebabCase => each(&mut values, str::to_shouty_kebab_case),
            Modifier::TrainCase => each(&mut values, str::to_train_case),
            Modifier::Replace(from, to) => each(&mut values, |val| val.replace(from, to)),
            Modifier::Substr(start, end) => each(&mut values, |val| {
                let start = start.unwrap_or(0);
                let end = end.unwrap_or(val.len());
                val[start..end].to_string()
            }),
            Modifier::Reverse => {
                if !list_mode {
                    each(&mut values, |val| val.chars().rev().collect::<String>());
                    continue;
                }
                values.reverse();
            }
            Modifier::Repeat(times) => {
                if list_mode {
                    values = values
                        .iter()
                        .cloned()
                        .cycle()
                        .take(values.len() * *times)
                        .collect();
                    continue;
                }

                each(&mut values, |val| val.repeat(*times));
            }
            Modifier::Split(pat) => {
                if list_mode {
                    values = values
                        .iter()
                        .flat_map(|val| val.split(pat))
                        .filter(|val| !val.is_empty())
                        .map(|val| val.to_owned())
                        .collect::<Vec<String>>();

                    continue;
                }

                values = values
                    .iter()
                    .flat_map(|v| v.split(pat).map(str::to_string))
                    .collect();
            }
            Modifier::SplitAt(mid) => {
                if list_mode {
                    values = values
                        .iter()
                        .flat_map(|val| {
                            let len = val.len();
                            // to avoid out of bounds
                            let mid = mid.min(&len);
                            let (a, b) = val.split_at(*mid);
                            [a, b]
                        })
                        .map(|val| val.to_owned())
                        .filter(|val| !val.is_empty())
                        .collect::<Vec<String>>();

                    continue;
                }

                values = values
                    .iter()
                    .flat_map(|v| {
                        let (left, right) = v.split_at(*mid);
                        [left.to_string(), right.to_string()]
                    })
                    .collect();
            }
            Modifier::Join(sep) => {
                let result = values.join(sep);
                values.clear();
                values.push(result);
            }
            Modifier::PadStart(total_width, pat) => {
                values.iter_mut().for_each(|val| {
                    let width: i32 = *total_width as i32 - (val.len() as i32);
                    // let width: i32 = (max(*width, pat.len())/pat.len()) as i32 - (val.len() as i32);
                    if width <= 0 {
                        return;
                    }
                    // this can happen when the user adds a pattern that is long, like `|_|` per instance and max remaning width for padding is 2
                    // per ex:
                    //------------------------------------------------01234567890
                    // oneStr | padstart { '|_|', 8 } should render  "|_oneStr"    -> remaining padding is 2, as  8 - 6 = 2
                    // oneStr | padstart { '|_|', 11 } should render "|_||_oneStr" -> remaining padding is 5, as 11 - 6 = 5
                    let mut padding = pat.repeat(max(*total_width, pat.len()) / pat.len())
                        [0..(width as usize)]
                        .to_string();
                    padding.push_str(val);
                    val.clear();
                    val.push_str(&padding);
                })
            }
            Modifier::PadEnd(total_width, pat) => {
                values.iter_mut().for_each(|val| {
                    let width: i32 =
                        (max(*total_width, pat.len()) / pat.len()) as i32 - (val.len() as i32);
                    if width <= 0 {
                        return;
                    }
                    // this can happen when the user adds a pattern that is long, like `|_|` per instance and max remaning width for padding is 2
                    // per ex:
                    //----------------------------------------------01234567890
                    // oneStr | padend { '|_|', 8 } should render  "oneStr|_"    -> remaining padding is 2, as  8 - 6 = 2
                    // oneStr | padend { '|_|', 11 } should render "oneStr|_||_" -> remaining padding is 5, as 11 - 6 = 5
                    val.push_str(
                        &pat.repeat(max(*total_width, pat.len()) / pat.len())[0..(width as usize)],
                    );
                });
            }
            Modifier::Slice(start, end) => {
                if list_mode {
                    let len = values.len() as i32;
                    let start = parse_pos(len, start.unwrap_or(0));
                    let end = parse_pos(len, end.unwrap_or(len));

                    if start >= end {
                        values = vec![];
                    } else {
                        values = values.get(start..end).unwrap_or_default().to_vec();
                    }
                    continue;
                }

                each(&mut values, |val| {
                    let len = val.len() as i32;
                    let start = parse_pos(len, start.unwrap_or(0));
                    let end = parse_pos(len, end.unwrap_or(len));

                    if start >= end {
                        return String::new();
                    }

                    val.get(start..end).unwrap_or_default().to_string()
                });
            }
            Modifier::Splice(output, start, end, replace_with) => {
                if list_mode {
                    let len = values.len() as i32;
                    let start = parse_pos(len, start.unwrap_or(0));
                    let end = parse_pos(len, end.unwrap_or(len));

                    if start > end {
                        values = vec![];
                    } else {
                        let removed = values.splice(start..end, replace_with.clone()).collect();

                        if let Output::Removed = output {
                            values = removed;
                        }
                    }
                    continue;
                }

                each(&mut values, |val| {
                    let len = val.len() as i32;
                    let start = parse_pos(len, start.unwrap_or(0));
                    let end = parse_pos(len, end.unwrap_or(len));

                    let replace_with = replace_with.clone().unwrap_or(String::new());

                    if start > end {
                        return String::new();
                    }

                    let removed = val.get(start..end).unwrap_or_default().to_string();

                    let mut val = val.to_string();
                    val.replace_range(start..end, &replace_with);

                    if let Output::Removed = output {
                        return removed;
                    }
                    val
                });
            }
        }
    }
    values.join("")
}

// couldnt think of a name...
fn parse_pos(len: i32, val: i32) -> usize {
    if val < 0 {
        (len + val) as usize
    } else {
        val.min(len) as usize
    }
}
