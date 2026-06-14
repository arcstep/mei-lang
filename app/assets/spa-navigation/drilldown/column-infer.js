  function sceneParamRowsetDatasetId(params) {
    if (!params || typeof params !== "object" || Array.isArray(params)) return "";
    return nonEmptyString(params.rowset_dataset_id, params.rowsetDatasetId);
  }

  function inferDrilldownColumnFormats(columns) {
    const formats = {};
    (Array.isArray(columns) ? columns : []).forEach((col) => {
      const name = String(col || "").trim();
      if (!name) return;
      if (/等级/.test(name)) {
        formats[name] = { tag: true };
        return;
      }
      if (/承办部门|主责单位/.test(name)) {
        formats[name] = { truncate: true, maxChars: 14 };
        return;
      }
      if (/办公地址|住所地址|注册地址/.test(name)) {
        formats[name] = { truncate: false, wrap: true };
        return;
      }
      if (/部门|单位|主责/.test(name)) {
        formats[name] = { truncate: true, maxChars: 18 };
        return;
      }
      if (/描述|事项|问题|表现|情况|名称|规则|依据|文件/.test(name)) {
        formats[name] = { truncate: true, maxChars: 24 };
      }
    });
    return formats;
  }

  function inferDrilldownColumnState(columns) {
    return {
      columns: (Array.isArray(columns) ? columns : []).map((key, order) => {
        const name = String(key || "").trim();
        if (!name) return { key: name, order };
        if (/等级/.test(name)) {
          return { key: name, order, width: 76, width_mode: "fixed", align: "center" };
        }
        if (/序号/.test(name)) {
          return { key: name, order, width: 64, width_mode: "fixed", align: "center" };
        }
        if (/类别/.test(name)) {
          return { key: name, order, width: 96, width_mode: "fixed" };
        }
        if (/^执法单位$/.test(name)) {
          return { key: name, order, width: 140, width_mode: "fixed" };
        }
        if (/办公地址|住所地址|注册地址/.test(name)) {
          return { key: name, order, width_mode: "content", wrap: true };
        }
        if (/承办部门|主责单位/.test(name)) {
          return { key: name, order, align: "left" };
        }
        return { key: name, order };
      }),
    };
  }
