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
    /// `use template` imports from the defining file (alias → import path).
    pub module_imports: BTreeMap<String, String>,
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
    /// Global name → def (app-first overwrite on layered load).
    by_name: BTreeMap<String, MacroDef>,
    /// import path → (template name → def); keeps every export in a module.
    by_module: BTreeMap<String, BTreeMap<String, MacroDef>>,
}

impl MacroRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_file(&mut self, rel_path: &str, file: &V2SourceFile) {
        let file_path = normalize_template_path(rel_path);
        let mut module_consts = BTreeMap::new();
        let mut module_imports = BTreeMap::new();
        for item in &file.items {
            match item {
                V2Item::ModuleConst { name, value } => {
                    module_consts.insert(name.clone(), value.clone());
                }
                V2Item::UseTemplate { path, alias } => {
                    let norm = normalize_template_path(path);
                    let import_name = alias
                        .clone()
                        .unwrap_or_else(|| norm.rsplit('/').next().unwrap_or(&norm).to_string());
                    module_imports.insert(import_name, norm);
                }
                _ => {}
            }
        }
        let mut module_map = BTreeMap::new();
        for item in &file.items {
            if let V2Item::TemplateDecl { name, params, body } = item {
                let def = MacroDef {
                    file_path: file_path.clone(),
                    name: name.clone(),
                    params: params.clone(),
                    body: body.clone(),
                    module_consts: module_consts.clone(),
                    module_imports: module_imports.clone(),
                };
                module_map.insert(name.clone(), def.clone());
                self.by_name.insert(name.clone(), def);
            }
        }
        if !module_map.is_empty() {
            self.by_module.insert(file_path, module_map);
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
        if let Some(module) = self.by_module.get(&norm) {
            // Prefer a template whose name matches the leaf path segment when present.
            let leaf = norm.rsplit('/').next().unwrap_or(norm.as_str());
            if let Some(def) = module.get(leaf) {
                return Some(def);
            }
            return module.values().next();
        }
        let leaf = norm.rsplit('/').next().unwrap_or(norm.as_str());
        self.by_name.get(leaf)
    }

    pub fn resolve_name(&self, name: &str) -> Option<&MacroDef> {
        self.by_name.get(name)
    }

    /// Resolve `alias.method` via import path: `imports[alias] → method` in that module.
    pub fn resolve_in_module(&self, import_path: &str, method: &str) -> Option<&MacroDef> {
        let norm = normalize_template_path(import_path);
        self.by_module
            .get(&norm)
            .and_then(|module| module.get(method))
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
                || rel_check.contains("/templates/")
            {
                continue;
            }
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let import_path = rel
            .trim_end_matches(".mei")
            .trim_end_matches("/mod")
            .to_string();
        let source = std::fs::read_to_string(path)?;
        let Ok(file) = mei_syntax::v2::parse_v2_source(&source) else {
            continue;
        };
        registry.register_file(&import_path, &file);
    }
    Ok(())
}

fn template_file_path(root: &Path, import_path: &str) -> PathBuf {
    let norm = normalize_template_path(import_path);
    let direct = root.join(format!("{norm}.mei"));
    if direct.is_file() {
        return direct;
    }
    root.join(norm).join("mod.mei")
}

pub fn normalize_template_path(path: &str) -> String {
    path.trim()
        .trim_matches('"')
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_end_matches(".mei")
        .trim_end_matches('/')
        .to_string()
}
