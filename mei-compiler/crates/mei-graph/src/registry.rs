use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mei_syntax::v2::{TemplateParam, V2Expr, V2Item, V2SourceFile};

#[derive(Debug, Clone)]
pub struct MacroDef {
    pub file_path: String,
    pub name: String,
    pub params: Vec<TemplateParam>,
    pub body: V2Expr,
    pub module_consts: BTreeMap<String, V2Expr>,
}

#[derive(Debug, Default, Clone)]
pub struct MacroRegistry {
    by_name: BTreeMap<String, MacroDef>,
    by_import_path: BTreeMap<String, String>,
}

impl MacroRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_file(&mut self, rel_path: &str, file: &V2SourceFile) {
        let file_path = normalize_template_path(rel_path);
        let mut module_consts = BTreeMap::new();
        for item in &file.items {
            if let V2Item::ModuleConst { name, value } = item {
                module_consts.insert(name.clone(), value.clone());
            }
        }
        let mut names = Vec::new();
        for item in &file.items {
            if let V2Item::TemplateDecl { name, params, body } = item {
                names.push(name.clone());
                self.by_name.insert(
                    name.clone(),
                    MacroDef {
                        file_path: file_path.clone(),
                        name: name.clone(),
                        params: params.clone(),
                        body: body.clone(),
                        module_consts: module_consts.clone(),
                    },
                );
            }
        }
        if names.len() == 1 {
            self.by_import_path
                .insert(file_path.clone(), names[0].clone());
        }
    }

    pub fn load_dir(root: &Path) -> std::io::Result<Self> {
        let mut registry = Self::new();
        if !root.is_dir() {
            return Ok(registry);
        }
        for entry in walkdir::WalkDir::new(root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "mei"))
        {
            let path = entry.path();
            let rel = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            let rel = rel.strip_suffix(".mei").unwrap_or(&rel).to_string();
            let source = std::fs::read_to_string(path)?;
            if let Ok(file) = mei_syntax::v2::parse_v2_source(&source) {
                registry.register_file(&rel, &file);
            }
        }
        Ok(registry)
    }

    pub fn resolve_path(&self, path: &str) -> Option<&MacroDef> {
        let norm = normalize_template_path(path);
        if let Some(name) = self.by_import_path.get(&norm) {
            return self.by_name.get(name);
        }
        let leaf = norm.rsplit('/').next().unwrap_or(norm.as_str());
        self.by_name.get(leaf)
    }

    pub fn resolve_name(&self, name: &str) -> Option<&MacroDef> {
        self.by_name.get(name)
    }

    pub fn resolve_qualified(&self, _alias: &str, method: &str) -> Option<&MacroDef> {
        self.by_name.get(method)
    }
}

pub fn normalize_template_path(path: &str) -> String {
    path.trim()
        .replace('\\', "/")
        .trim_end_matches(".mei")
        .to_string()
}

pub fn template_file_path(stock_templates: &Path, import_path: &str) -> PathBuf {
    let rel = import_path.trim().trim_matches('"');
    let mut path = stock_templates.to_path_buf();
    for segment in rel.split('/') {
        path.push(segment);
    }
    if path.extension().is_none() {
        path.set_extension("mei");
    }
    path
}
