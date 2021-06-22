use crate::ty::*;
use crate::{generic, sig};
use std::collections::HashMap;
use std::lazy::SyncLazy;

pub static INTRINSICS: SyncLazy<HashMap<&'static str, Ty>> = SyncLazy::new(|| {
    let mut map = HashMap::new();
    let int32 = Ty::int(Integer::I32, true);
    let isize = Ty::int(Integer::ISize, true);
    let boolean = Ty::int(Integer::I8, false);
    let ptr = Ty::unit().ptr();

    map.insert("add_i32", sig!(int32, int32 => int32));
    map.insert("sub_i32", sig!(int32, int32 => int32));
    map.insert("mul_i32", sig!(int32, int32 => int32));
    map.insert("div_i32", sig!(int32, int32 => int32));
    map.insert("rem_i32", sig!(int32, int32 => int32));
    map.insert("eq_i32", sig!(int32, int32 => boolean));
    map.insert("ne_i32", sig!(int32, int32 => boolean));
    map.insert("lt_i32", sig!(int32, int32 => boolean));
    map.insert("le_i32", sig!(int32, int32 => boolean));
    map.insert("gt_i32", sig!(int32, int32 => boolean));
    map.insert("ge_i32", sig!(int32, int32 => boolean));

    map.insert("ptr_offset", generic!(t:Type in sig!(t.ptr(), isize => t.ptr())));

    map
});
