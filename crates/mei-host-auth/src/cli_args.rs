use std::path::PathBuf;

use clap::{Args, Subcommand};

#[derive(Args, Clone, Debug)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub command: AuthCommand,
}

#[derive(Subcommand, Clone, Debug)]
pub enum AuthCommand {
    /// 生成 JWT 密钥与登录 RSA 密钥对（写入 `.mei/local/hosts/*.state.json`，不涉及用户密码）
    EnsureKeys(AuthEnsureKeysArgs),
    /// 一次性初始化 super/admin/guest 用户并生成临时密码（不使用固定默认密码）
    BootstrapUsers(AuthBootstrapUsersArgs),
    /// 新增或更新单个用户；密码通过 stdin 传入（禁止命令行明文密码）
    AddUser(AuthAddUserArgs),
    /// 禁用用户（写入 `disabled=true`）
    DisableUser(AuthSetUserEnabledArgs),
    /// 启用用户（写入 `disabled=false`）
    EnableUser(AuthSetUserEnabledArgs),
    RotateKeys(AuthRotateKeysArgs),
    /// 从标准输入读取密码并输出 Argon2 哈希（供写入配置 `passwordHash`，禁止在命令行传明文密码）
    HashPassword(AuthHashPasswordArgs),
    Describe(AuthDescribeArgs),
}

#[derive(Args, Clone, Debug)]
pub struct AuthEnsureKeysArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone, Debug)]
pub struct AuthRotateKeysArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone, Debug)]
pub struct AuthBootstrapUsersArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long, default_value = "super")]
    pub super_username: String,
    #[arg(long, default_value = "超级管理员")]
    pub super_profile: String,
    #[arg(long, default_value = "admin")]
    pub admin_username: String,
    #[arg(long, default_value = "管理员")]
    pub admin_profile: String,
    #[arg(long, default_value = "guest")]
    pub guest_username: String,
    #[arg(long, default_value = "访客")]
    pub guest_profile: String,
    #[arg(long = "guest-app-allow")]
    pub guest_app_allow: Vec<String>,
    #[arg(long = "guest-scene-allow", help = "格式: app_id:scene_id")]
    pub guest_scene_allow: Vec<String>,
    /// 从 stdin 读取统一初始密码（super/admin/guest 共用）；未指定时为各账号随机生成。
    #[arg(long)]
    pub default_password_stdin: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone, Debug)]
pub struct AuthAddUserArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub username: String,
    #[arg(long, default_value = "guest", value_parser = ["super", "admin", "guest"])]
    pub role: String,
    #[arg(long, default_value = "")]
    pub profile: String,
    #[arg(long = "app-allow")]
    pub app_allow: Vec<String>,
    #[arg(long = "scene-allow", help = "格式: app_id:scene_id")]
    pub scene_allow: Vec<String>,
    /// 必须显式声明从 stdin 读取密码，避免误将明文放进命令行参数。
    #[arg(long)]
    pub password_stdin: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone, Debug)]
pub struct AuthSetUserEnabledArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub username: String,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone, Debug)]
pub struct AuthHashPasswordArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone, Debug)]
pub struct AuthDescribeArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long)]
    pub json: bool,
}

/// Legacy host-web CLI uses `--source-root`; map to workspace.
#[derive(Args, Clone, Debug)]
pub struct LegacyAuthEnsureKeysArgs {
    #[arg(long)]
    pub source_root: PathBuf,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone, Debug)]
pub struct LegacyAuthRotateKeysArgs {
    #[arg(long)]
    pub source_root: PathBuf,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone, Debug)]
pub struct LegacyAuthBootstrapUsersArgs {
    #[arg(long)]
    pub source_root: PathBuf,
    #[arg(long, default_value = "super")]
    pub super_username: String,
    #[arg(long, default_value = "超级管理员")]
    pub super_profile: String,
    #[arg(long, default_value = "admin")]
    pub admin_username: String,
    #[arg(long, default_value = "管理员")]
    pub admin_profile: String,
    #[arg(long, default_value = "guest")]
    pub guest_username: String,
    #[arg(long, default_value = "访客")]
    pub guest_profile: String,
    #[arg(long = "guest-app-allow")]
    pub guest_app_allow: Vec<String>,
    #[arg(long = "guest-scene-allow", help = "格式: app_id:scene_id")]
    pub guest_scene_allow: Vec<String>,
    #[arg(long)]
    pub default_password_stdin: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone, Debug)]
pub struct LegacyAuthAddUserArgs {
    #[arg(long)]
    pub source_root: PathBuf,
    #[arg(long)]
    pub username: String,
    #[arg(long, default_value = "guest", value_parser = ["super", "admin", "guest"])]
    pub role: String,
    #[arg(long, default_value = "")]
    pub profile: String,
    #[arg(long = "app-allow")]
    pub app_allow: Vec<String>,
    #[arg(long = "scene-allow", help = "格式: app_id:scene_id")]
    pub scene_allow: Vec<String>,
    #[arg(long)]
    pub password_stdin: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone, Debug)]
pub struct LegacyAuthSetUserEnabledArgs {
    #[arg(long)]
    pub source_root: PathBuf,
    #[arg(long)]
    pub username: String,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone, Debug)]
pub struct LegacyAuthDescribeArgs {
    #[arg(long)]
    pub source_root: PathBuf,
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand, Clone, Debug)]
pub enum LegacyAuthCommand {
    EnsureKeys(LegacyAuthEnsureKeysArgs),
    BootstrapUsers(LegacyAuthBootstrapUsersArgs),
    AddUser(LegacyAuthAddUserArgs),
    DisableUser(LegacyAuthSetUserEnabledArgs),
    EnableUser(LegacyAuthSetUserEnabledArgs),
    RotateKeys(LegacyAuthRotateKeysArgs),
    HashPassword(AuthHashPasswordArgs),
    Describe(LegacyAuthDescribeArgs),
}

#[derive(Args, Clone, Debug)]
pub struct LegacyAuthArgs {
    #[command(subcommand)]
    pub command: LegacyAuthCommand,
}
