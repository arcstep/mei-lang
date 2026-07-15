# archive/mei-host-web — 待手工清理

| 项 | 内容 |
|----|------|
| 来源 | `server` 包 `[[bin]] mei-host-web` 入口 |
| 迁出日期 | 2026-07-15 |
| 状态 | **待手工清理**；产品宿主已切到 `mei-host-shell` |
| 说明 | `server` 内仍有历史 HTTP 面代码（供 toolchain 库编译）；完整剥离另开任务 |

主树已移除该 bin；请用：`cargo run -p mei-host-shell -- serve --workspace <ws>`。
