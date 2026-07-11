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

/// Template search roots for `use template` (app-first, then workspace stock).
#[derive(Debug, Clone)]
pub struct TemplateRoots {
    pub app_templates: PathBuf,
    pub app_src: PathBuf,
    pub stock: PathBuf,
}

impl TemplateRoots {
    pub fn from_app_and_stock(app_root: &Path, stock: PathBuf) -> Self {
        Self {
            app_templates: app_root.join("src/templates"),
            app_src: app_root.join("src"),
            stock,
        }
    }

    pub fn stock_only(stock: PathBuf) -> Self {
        Self {
            app_templates: PathBuf::new(),
            app_src: PathBuf::new(),
            stock,
        }
    }

    /// Resolve import path to an on-disk `.mei` file (app templates → app src → stock).
    pub fn resolve_file(&self, import_path: &str) -> Option<PathBuf> {
        for root in self.search_roots() {
            if !root.is_dir() {
                continue;
            }
            let candidate = template_file_path(root, import_path);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }

    fn search_roots(&self) -> impl Iterator<Item = &Path> {
        [
            self.app_templates.as_path(),
            self.app_src.as_path(),
            self.stock.as_path(),
        ]
        .into_iter()
        .filter(|p| !p.as_os_str().is_empty())
    }
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
        if !names.is_empty() {
            // Multi-template files (e.g. geometry.mei) must still resolve by import path
            // so `use template "scene/.../geometry" as geo` succeeds; qualified calls
            // then resolve via template name (`geo.focus_inset`).
            self.by_import_path
                .insert(file_path.clone(), names[0].clone());
        }
    }

    pub fn load_dir(root: &Path) -> std::io::Result<Self> {
        let mut registry = Self::new();
        load_dir_into(&mut registry, root, false)?;
        Ok(registry)
    }

    /// Load stock first, then app `src` (excluding `templates/`), then `src/templates`.
    /// Later layers overwrite same import path / template name (app wins).
    pub fn load_layered(roots: &TemplateRoots) -> std::io::Result<Self> {
        let mut registry = Self::new();
        if roots.stock.is_dir() {
            load_dir_into(&mut registry, &roots.stock, false)?;
        }
        if roots.app_src.is_dir() {
            load_dir_into(&mut registry, &roots.app_src, true)?;
        }
        if roots.app_templates.is_dir() {
            load_dir_into(&mut registry, &roots.app_templates, false)?;
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

fn load_dir_into(
    registry: &mut MacroRegistry,
    root: &Path,
    skip_templates_subdir: bool,
) -> std::io::Result<()> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "mei"))
    {
        let path = entry.path();
        if skip_templates_subdir {
            let rel_check = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            if rel_check == "templates"
                || rel_check.starts_with("templates/")
                || rel_check
                    .split('/')
                    .next()
                    .is_some_and(|seg| seg == "templates")
            {
                continue;
            }
        }
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
    Ok(())
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
        if segment.is_empty() || segment == "." {
            continue;
        }
        path.push(segment);
    }
    if path.extension().is_none() {
        path.set_extension("mei");
    }
    path
}
