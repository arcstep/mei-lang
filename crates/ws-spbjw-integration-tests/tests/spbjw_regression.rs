use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, source_root, zhifa_app_root,
    CompileOptions,
};

#[path = "spbjw_regression/t1.rs"]
mod t1;
#[path = "spbjw_regression/t2.rs"]
mod t2;
#[path = "spbjw_regression/t3.rs"]
mod t3;
#[path = "spbjw_regression/t4.rs"]
mod t4;
#[path = "spbjw_regression/t5.rs"]
mod t5;
#[path = "spbjw_regression/t6.rs"]
mod t6;
#[path = "spbjw_regression/t7.rs"]
mod t7;
#[path = "spbjw_regression/t8.rs"]
mod t8;
#[path = "spbjw_regression/t9.rs"]
mod t9;
#[path = "spbjw_regression/t10.rs"]
mod t10;
#[path = "spbjw_regression/t11.rs"]
mod t11;
#[path = "spbjw_regression/t12.rs"]
mod t12;
#[path = "spbjw_regression/t13.rs"]
mod t13;
#[path = "spbjw_regression/t14.rs"]
mod t14;
#[path = "spbjw_regression/t15.rs"]
mod t15;
#[path = "spbjw_regression/t16.rs"]
mod t16;
#[path = "spbjw_regression/t17.rs"]
mod t17;
#[path = "spbjw_regression/t18.rs"]
mod t18;
#[path = "spbjw_regression/t19.rs"]
mod t19;
#[path = "spbjw_regression/t20.rs"]
mod t20;
#[path = "spbjw_regression/t21.rs"]
mod t21;
#[path = "spbjw_regression/t22.rs"]
mod t22;
#[path = "spbjw_regression/t23.rs"]
mod t23;
#[path = "spbjw_regression/t24.rs"]
mod t24;
