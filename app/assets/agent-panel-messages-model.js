/**
 * 消息纯逻辑：工具块摘要、权限阻塞解析、normalizeMessage、剪贴板。
 * 须先于 agent-panel-messages.js 拼接；由 install 通过 globalThis.MeiAgentPanelMessagesModel 调用。
 */
(function (global) {
  "use strict";

  const M = {
    makeTextBlock: function (label, content, type, collapsed) {
      const text = String(content || "").trim();
      if (!text) return null;
      return {
        type: String(type || "text"),
        label: String(label || ""),
        content: text,
        collapsed: collapsed === true,
      };
    },

    toolBlockLabel: function (part) {
      const tool = part && part.tool ? part.tool : null;
      if (!tool) return "工具调用";
      const name = String(tool.tool || "").trim() || "unknown";
      const fp = String(tool.input_path || "").trim();
      const title = String(tool.title || "").trim();
      if (name === "read_file" && fp) return "read_file · path=" + fp;
      if (name === "skill_read" && fp) return "skill_read · path=" + fp;
      if (name === "resource_get" && fp) return "resource_get · id=" + fp;
      if (name === "resource_list") return "resource_list";
      if (name === "resource_runtime_peek") return "resource_runtime_peek";
      if (name === "propose_session_patch") return "propose_session_patch";
      if (name === "skill_list") return "skill_list";
      if (fp) return name + " · filePath=" + fp;
      if (title && title !== name) return name + " · " + title;
      return name;
    },

    formatToolPart: function (part) {
      const tool = part && part.tool ? part.tool : null;
      if (!tool) return null;
      const name = String(tool.tool || "unknown");
      const lines = [];
      lines.push("工具: " + name);
      const fp = String(tool.input_path || "").trim();
      if (name === "read_file" && fp) lines.push("参数 path: " + fp);
      else if (name === "skill_read" && fp) lines.push("参数 path: " + fp);
      else if (name === "resource_get" && fp) lines.push("参数 id: " + fp);
      else if (fp) lines.push("参数 filePath: " + fp);
      lines.push("状态: " + String(tool.status || "pending"));
      const cid = String(tool.call_id || "").trim();
      if (cid) lines.push("call_id: " + cid);
      const title = String(tool.title || "").trim();
      if (title) lines.push("标题: " + title);
      if (tool.output) lines.push("输出:\n" + String(tool.output));
      if (tool.error) lines.push("错误:\n" + String(tool.error));
      return lines.join("\n");
    },

    looksLikeSkillPath: function (path) {
      return String(path || "").replaceAll("\\", "/").includes("/.mei/skills/meilang-author");
    },

    blockedPermissionNoticeFromData: function (data) {
      const permissionId = String((data && data.permission_id) || "").trim();
      const permission = String((data && data.permission) || "unknown").trim() || "unknown";
      const patterns = Array.isArray(data && data.patterns)
        ? data.patterns
            .map(function (item) {
              return String(item || "").trim();
            })
            .filter(Boolean)
        : [];
      const rawPath = String((data && data.path) || "").trim();
      const path = rawPath || (patterns.length > 0 ? patterns[0] : "");
      const requiresAdmin = !!(data && data.requires_admin);
      const message = String((data && data.message) || "").trim();
      return {
        id: permissionId || "path:" + (path || permission || "unknown"),
        permissionId: permissionId,
        permission: permission,
        path: path,
        patterns: patterns,
        requiresAdmin: requiresAdmin,
        message: message,
      };
    },

    blockedPermissionNoticeFromRunningRead: function (messageId, part) {
      const tool = part && part.tool ? part.tool : null;
      const path = String((tool && tool.input_path) || "").trim();
      const id = String((part && part.part_id) || "") || String(messageId || "");
      if (!path) return null;
      if (M.looksLikeSkillPath(path)) {
        return {
          id: "running-read:" + id,
          permissionId: "",
          permission: "external_directory",
          path: path,
          patterns: [path],
          requiresAdmin: true,
          message:
            "系统尝试读取 MeiLang skill 目录但当前未获授权。请在权限提示中批准，或请管理员检查 external_directory 策略。",
        };
      }
      return {
        id: "running-read:" + id,
        permissionId: "",
        permission: "external_directory",
        path: path,
        patterns: [path],
        requiresAdmin: true,
        message:
          "检测到会话尝试访问未授权目录。请先检查你输入的目标路径；若这是系统预期目录，请联系管理员处理白名单。",
      };
    },

    blockedPermissionBody: function (notice) {
      const lines = [];
      lines.push("类型: 权限阻塞");
      lines.push("permission: " + String(notice.permission || "unknown"));
      if (notice.permissionId) lines.push("permission_id: " + String(notice.permissionId));
      if (notice.path) lines.push("目录: " + String(notice.path));
      if (notice.patterns && notice.patterns.length > 0) {
        lines.push("匹配模式:");
        notice.patterns.forEach(function (pattern) {
          lines.push("- " + String(pattern));
        });
      }
      if (notice.message) lines.push("说明: " + String(notice.message));
      lines.push(
        notice.requiresAdmin
          ? "建议: 若目录正确，请联系管理员；若目录异常，请修正你的任务路径。"
          : "建议: 请检查当前任务与目录范围。",
      );
      return lines.join("\n");
    },

    mergeBlockedPermissionNotices: function (primary, fallback) {
      const merged = [];
      const seen = new Set();
      function addList(list) {
        (Array.isArray(list) ? list : []).forEach(function (item) {
          if (!item || typeof item !== "object") return;
          const id = String(item.id || "").trim();
          if (!id || seen.has(id)) return;
          seen.add(id);
          merged.push(item);
        });
      }
      addList(primary);
      addList(fallback);
      return merged;
    },

    blockedPermissionFingerprint: function (notices) {
      return M.mergeBlockedPermissionNotices(notices, [])
        .map(function (item) {
          return [
            String(item && item.id || ""),
            String(item && item.permissionId || ""),
            String(item && item.path || ""),
          ].join("|");
        })
        .filter(Boolean)
        .sort()
        .join("||");
    },

    copyText: function (text) {
      const value = String(text || "");
      if (!value) return Promise.resolve();
      if (navigator.clipboard && window.isSecureContext) {
        return navigator.clipboard.writeText(value);
      }
      return new Promise(function (resolve, reject) {
        try {
          const temp = document.createElement("textarea");
          temp.value = value;
          temp.setAttribute("readonly", "readonly");
          temp.style.position = "fixed";
          temp.style.left = "-9999px";
          temp.style.top = "-9999px";
          document.body.appendChild(temp);
          temp.select();
          document.execCommand("copy");
          document.body.removeChild(temp);
          resolve();
        } catch (error) {
          reject(error);
        }
      });
    },

    normalizeMessage: function (raw) {
      const partsRaw = Array.isArray(raw && raw.parts) ? raw.parts : [];
      const parts = partsRaw.slice();
      parts.sort(function (a, b) {
        const ao = Number(a && a.sort_order);
        const bo = Number(b && b.sort_order);
        if (Number.isFinite(ao) && Number.isFinite(bo) && ao !== bo) return ao - bo;
        return 0;
      });
      const role = String((raw && raw.role) || "assistant");
      const blocks = [];
      function pushTextBlock(text) {
        const t = String(text || "").trim();
        if (!t) return;
        const tb = M.makeTextBlock("", t, "text");
        if (tb) blocks.push(tb);
      }
      function pushReasoningBlock(text) {
        const t = String(text || "").trim();
        if (!t) return;
        const rb = M.makeTextBlock("思考（可折叠调试）", t, "reasoning", true);
        if (rb) blocks.push(rb);
      }
      parts.forEach(function (part) {
        const type = String((part && part.part_type) || "");
        if (type === "text") {
          pushTextBlock(part && part.text ? part.text : "");
          return;
        }
        if (type === "reasoning") {
          pushReasoningBlock(part && part.text ? part.text : "");
          return;
        }
        if (type === "tool") {
          const toolBody = M.formatToolPart(part);
          if (toolBody) {
            const label = M.toolBlockLabel(part);
            const block = M.makeTextBlock(label, toolBody, "tool", true);
            if (block) blocks.push(block);
          }
          return;
        }
        if (type === "patch") {
          const patchText = String(part && part.text ? part.text : "").trim();
          if (patchText) {
            const pb = M.makeTextBlock("代码补丁", patchText, "patch", true);
            if (pb) blocks.push(pb);
          }
          return;
        }
        if (part && part.raw) {
          const debugBody = JSON.stringify(part.raw, null, 2);
          const block = M.makeTextBlock("结构化片段", debugBody, "debug", true);
          if (block) blocks.push(block);
        }
      });
      const body =
        blocks.length > 0
          ? blocks
              .map(function (block) {
                return (block.label ? "[" + block.label + "]\n" : "") + block.content;
              })
              .join("\n\n")
          : "(空消息)";
      return {
        id: String((raw && raw.message_id) || ""),
        role: role,
        body: body,
        blocks: blocks,
        time: new Date().toLocaleTimeString("zh-CN", {
          hour: "2-digit",
          minute: "2-digit",
          second: "2-digit",
        }),
        actions: [],
      };
    },

    inferAgentModeFromRawMessage: function (raw) {
      if (!raw || String(raw.role || "") !== "assistant") return null;
      const parts = Array.isArray(raw.parts) ? raw.parts : [];
      const hasPatchPart = parts.some(function (part) {
        return String(part && part.part_type ? part.part_type : "") === "patch";
      });
      return hasPatchPart ? "build" : null;
    },

    deriveBlockedNoticesFromRawMessages: function (rawMessages) {
      const notices = [];
      (Array.isArray(rawMessages) ? rawMessages : []).forEach(function (raw) {
        if (!raw || String(raw.role || "") !== "assistant") return;
        const messageId = String(raw.message_id || "");
        const parts = Array.isArray(raw.parts) ? raw.parts : [];
        parts.forEach(function (part) {
          if (!part || String(part.part_type || "") !== "tool") return;
          const tool = part.tool || null;
          if (!tool) return;
          if (String(tool.tool || "") !== "read") return;
          if (String(tool.status || "") !== "running") return;
          const notice = M.blockedPermissionNoticeFromRunningRead(messageId, part);
          if (notice) notices.push(notice);
        });
      });
      return notices;
    },
  };

  global.MeiAgentPanelMessagesModel = M;
})(typeof globalThis !== "undefined" ? globalThis : window);
