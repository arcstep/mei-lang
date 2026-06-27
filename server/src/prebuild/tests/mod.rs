use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::prebuild::*;
use mei_lang_kernel::CompiledSceneRoute;
use serde_json::json;

pub(crate) use c1::test_outcome;

#[path = "c1.rs"]
mod c1;
#[path = "c2.rs"]
mod c2;
#[path = "c3.rs"]
mod c3;
#[path = "c4.rs"]
mod c4;
#[path = "c5.rs"]
mod c5;
#[path = "c6.rs"]
mod c6;
