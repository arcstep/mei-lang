AI-native scene orchestration language for building apps from world models, UI components, and capability modules.

## 本地启动

### 1. 配置 `.env`（OpenAI 兼容）

`mei-lang` 内置 agent 直接读取 OpenAI 兼容配置。

最小示例：

```bash
OPENAI_IMITATORS=QWEN
QWEN_BASE_URL=https://dashscope.aliyuncs.com/compatible-mode/v1
QWEN_API_KEY=your_api_key
QWEN_COMPLETION_MODEL=qwen-max
```

### 2. 启动 `mei-lang`

```bash
cd mei-lang
cargo run -p mei-lang-server -- serve
```

默认行为：

- `mei-lang` 监听 **http://127.0.0.1:3000**
- 源码根目录 **`--source-root`** 未传时默认为仓库内 **`../workspaces`**
- 启动后直接使用内置 agent（按 `.env` 中 OpenAI 兼容配置连模型服务）

浏览器打开根路径即可；应用页面路由形如 **`/apps/manage/<app_id>`**。

## 停止服务

- 停止 `mei-lang`：在 `mei serve` 所在终端里按 `Ctrl+C`

## 最少配置

至少需要一组 OpenAI 兼容补全模型配置（见上面的 `.env` 示例）。若有多供应商，可通过 `OPENAI_IMITATORS` 追加前缀并配置对应的 `*_BASE_URL` / `*_API_KEY` / `*_COMPLETION_MODEL`。
