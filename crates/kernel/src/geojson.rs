//! GeoJSON → tabular rows for dataset sources (`ds.geojson`).
//! Drops coordinate arrays; keeps feature type, properties id/name, and geometry type.

use serde_json::{json, Map, Value};

/// Parse GeoJSON text into flat table rows (`id`, `name`, `type`, `geometry-type`).
pub fn parse_geojson_rows(raw: &str) -> Result<Vec<Value>, serde_json::Error> {
    let root: Value = serde_json::from_str(raw)?;
    Ok(rows_from_geojson_value(&root))
}

/// Extract tabular rows from an already-parsed JSON value.
pub fn rows_from_geojson_value(root: &Value) -> Vec<Value> {
    feature_values(root)
        .iter()
        .filter_map(|feature| flatten_feature_row(feature))
        .collect()
}

fn feature_values(root: &Value) -> Vec<Value> {
    match root.get("type").and_then(Value::as_str) {
        Some("FeatureCollection") => root
            .get("features")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        Some("Feature") => vec![root.clone()],
        _ => root.as_array().cloned().unwrap_or_default(),
    }
}

fn flatten_feature_row(feature: &Value) -> Option<Value> {
    let object = feature.as_object()?;
    let mut row = Map::new();

    let feature_type = object
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("Feature");
    row.insert("type".to_string(), json!(feature_type));

    if let Some(properties) = object.get("properties").and_then(Value::as_object) {
        if let Some(id) = properties.get("id") {
            row.insert("id".to_string(), id.clone());
        }
        if let Some(name) = properties.get("name") {
            row.insert("name".to_string(), name.clone());
        }
    }

    if let Some(geometry_type) = object
        .get("geometry")
        .and_then(Value::as_object)
        .and_then(|geometry| geometry.get("type"))
    {
        row.insert("geometry-type".to_string(), geometry_type.clone());
    }

    Some(Value::Object(row))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_collection_flattens_without_coordinates() {
        let raw = r#"{
            "type": "FeatureCollection",
            "features": [
                {
                    "type": "Feature",
                    "properties": { "id": "1", "name": "A" },
                    "geometry": { "type": "Polygon", "coordinates": [[[0,0]]] }
                },
                {
                    "type": "Feature",
                    "properties": { "id": "2", "name": "B" },
                    "geometry": { "type": "MultiPolygon", "coordinates": [] }
                }
            ]
        }"#;
        let rows = parse_geojson_rows(raw).expect("parse");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["id"], "1");
        assert_eq!(rows[0]["name"], "A");
        assert_eq!(rows[0]["type"], "Feature");
        assert_eq!(rows[0]["geometry-type"], "Polygon");
        assert!(rows[0].get("coordinates").is_none());
        assert!(rows[0].get("geometry").is_none());
    }
}
