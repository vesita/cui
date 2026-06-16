use case::CaseExt;
use syn::{GenericArgument, Ident, PathArguments, Type};

/// 将类型名（最后一个路径段）转为 snake_case。
/// 例如 `TrafficLight` → `traffic_light`。
pub(crate) fn type_to_snake(t: &Type) -> String {
    let ident = type_last_ident(t);
    ident.to_string().to_snake()
}

/// 获取类型的最后一个路径段标识符。
///
/// # Panics
/// 类型必须是 `Type::Path`（宏输入中只有路径类型合法）。
pub(crate) fn type_last_ident(t: &Type) -> &Ident {
    match t {
        Type::Path(p) => &p.path.segments.last().unwrap().ident,
        _ => unreachable!("transitions! 宏只接受路径类型作为消息"),
    }
}

/// 获取类型中的泛型参数。
///
/// # Panics
/// 类型必须是 `Type::Path`（宏输入中只有路径类型合法）。
pub(crate) fn type_args(t: &Type) -> Vec<GenericArgument> {
    match t {
        Type::Path(p) => match &p.path.segments.last().unwrap().arguments {
            PathArguments::AngleBracketed(a) => a.args.iter().cloned().collect(),
            PathArguments::None => vec![],
            _ => unreachable!("不支持的路径参数形式"),
        },
        _ => unreachable!("transitions! 宏只接受路径类型作为消息"),
    }
}

/// 将泛型参数排序：生命周期在前，其余在后。
pub(crate) fn reorder_type_arguments(mut args: Vec<GenericArgument>) -> Vec<GenericArgument> {
    let mut lifetimes = Vec::new();
    let mut others = Vec::new();
    for arg in args.drain(..) {
        if matches!(arg, GenericArgument::Lifetime(_)) {
            lifetimes.push(arg);
        } else {
            others.push(arg);
        }
    }
    lifetimes.extend(others);
    lifetimes
}
