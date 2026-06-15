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
      if (/部门|单位|机构|主责/.test(name)) {
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
        if (/部门|单位|机构|主责/.test(name)) {
          return { key: name, order, align: "left" };
        }
        return { key: name, order };
      }),
    };
  }
