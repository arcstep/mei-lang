  function sceneParamRowsetDatasetId(params) {
    if (!params || typeof params !== "object" || Array.isArray(params)) return "";
    return nonEmptyString(params.rowset_dataset_id, params.rowsetDatasetId);
  }

  function isIdentifierColumn(name) {
    const text = String(name || "").trim();
    if (!text || /是否/.test(text)) return false;
    return /ID$/i.test(text) || /编号$/.test(text) || /编码$/.test(text);
  }

  function isSerialNumberColumn(name) {
    const text = String(name || "").trim();
    return Boolean(text) && (text === "序号" || text.endsWith("序号"));
  }

  function inferDrilldownColumnFormats(columns) {
    const formats = {};
    (Array.isArray(columns) ? columns : []).forEach((col) => {
      const name = String(col || "").trim();
      if (!name) return;
      if (isSerialNumberColumn(name)) {
        formats[name] = { align: "center", type: "text" };
        return;
      }
      if (isIdentifierColumn(name)) {
        formats[name] = { truncate: false };
        return;
      }
      if (name === "风险等级") {
        formats[name] = { kind: "risk_level_blocks", tag: false };
        return;
      }
      if (name === "预警等级" || name === "级别" || name === "level") {
        formats[name] = { kind: "warning_level_block", tag: false };
        return;
      }
      if (/等级/.test(name)) {
        formats[name] = { kind: "warning_level_block", tag: false };
        return;
      }
      if (/部门|单位|机构|主责/.test(name)) {
        formats[name] = { truncate: false };
        return;
      }
      if (/描述|事项|问题|表现|情况|名称|规则|依据|文件|备注|处置/.test(name)) {
        formats[name] = { truncate: false };
      }
    });
    return formats;
  }

  function inferDrilldownColumnState(columns) {
    return {
      columns: (Array.isArray(columns) ? columns : []).map((key, order) => {
        const name = String(key || "").trim();
        if (!name) return { key: name, order };
        if (isSerialNumberColumn(name)) {
          return { key: name, order, width: 72, width_mode: "fixed", align: "center" };
        }
        if (isIdentifierColumn(name)) {
          return { key: name, order, width_mode: "fixed", align: "left" };
        }
        if (name === "风险等级") {
          return { key: name, order, width: 180, width_mode: "fixed", align: "center" };
        }
        if (name === "预警等级" || name === "级别" || name === "level" || /等级/.test(name)) {
          return { key: name, order, width: 72, width_mode: "fixed", align: "center" };
        }
        if (/部门|单位|机构|主责/.test(name)) {
          return { key: name, order, align: "left" };
        }
        return { key: name, order };
      }),
    };
  }

  /** 明细表有序号列时，默认按序号升序（段对齐排序由服务端 serial sort 负责）。 */
  function inferDrilldownDefaultSort(columns) {
    const names = (Array.isArray(columns) ? columns : [])
      .map((item) => String(item || "").trim())
      .filter(Boolean);
    const serial = names.find((name) => isSerialNumberColumn(name));
    return serial ? [{ field: serial, direction: "asc" }] : [];
  }
