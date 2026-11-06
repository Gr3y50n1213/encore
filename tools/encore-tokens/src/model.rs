//! Reads `tokens.json` into a typed tree that every emitter shares.
//!
//! Rules:
//! - Keys starting with `$` (`$source`, `$status`, `$description`, `$units`) are
//!   documentation. They become doc comments on the group, never constants.
//! - Objects become nested groups.
//! - Lists hold one kind of value. Whole numbers mixed with decimals become decimals;
//!   any other mix is an error, so a mistake in tokens.json fails loudly.
//! - A list of objects becomes a list of records. Every object needs the same keys.
//! - `null` and empty lists are errors: their type cannot be known.

use serde_json::{Map, Value};

pub type Result<T> = std::result::Result<T, String>;

/// The type of one value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Int,
    Float,
    Bool,
    Str,
}

/// One value.
#[derive(Debug, Clone, PartialEq)]
pub enum Scalar {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
}

impl Scalar {
    pub fn kind(&self) -> Kind {
        match self {
            Scalar::Int(_) => Kind::Int,
            Scalar::Float(_) => Kind::Float,
            Scalar::Bool(_) => Kind::Bool,
            Scalar::Str(_) => Kind::Str,
        }
    }

    /// Whole numbers widen to decimals when they share a list or field with decimals.
    fn widen(self, kind: Kind) -> Scalar {
        match (self, kind) {
            (Scalar::Int(i), Kind::Float) => Scalar::Float(i as f64),
            (s, _) => s,
        }
    }
}

/// A constant: one value or a list of values of one kind.
#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    One(Scalar),
    List(Kind, Vec<Scalar>),
}

/// The declared type of a record field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    One(Kind),
    List(Kind),
}

/// A list of objects that share the same keys.
#[derive(Debug, Clone, PartialEq)]
pub struct Records {
    pub fields: Vec<(String, FieldType)>,
    pub rows: Vec<Vec<Field>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    Field(Field),
    Group(Group),
    Records(Records),
}

/// An object in tokens.json.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub docs: Vec<String>,
    pub entries: Vec<(String, Entry)>,
}

pub fn parse(json: &str) -> Result<Group> {
    let value: Value =
        serde_json::from_str(json).map_err(|e| format!("tokens.json is not valid JSON: {e}"))?;
    match value {
        Value::Object(map) => parse_group(&map, "tokens"),
        _ => Err("tokens.json must be a JSON object at the top".into()),
    }
}

fn parse_group(map: &Map<String, Value>, path: &str) -> Result<Group> {
    let mut docs = Vec::new();
    let mut entries = Vec::new();
    for (key, value) in map {
        let here = format!("{path}.{key}");
        if let Some(label) = key.strip_prefix('$') {
            docs.push(doc_line(label, value));
            continue;
        }
        let entry = match value {
            Value::Object(inner) => {
                let type_name = crate::names::pascal(key);
                if crate::names::BUILTIN_TYPES.contains(&type_name.as_str()) {
                    return Err(format!(
                        "{here}: a group named {type_name} would hide the built-in type \
                         {type_name} in the generated code; rename the group"
                    ));
                }
                Entry::Group(parse_group(inner, &here)?)
            }
            Value::Array(items) => parse_list(items, &here)?,
            other => Entry::Field(Field::One(scalar(other, &here)?)),
        };
        entries.push((key.clone(), entry));
    }
    Ok(Group { docs, entries })
}

fn doc_line(label: &str, value: &Value) -> String {
    let text = match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    match label {
        "description" => text,
        _ => {
            let mut chars = label.chars();
            let title: String = chars
                .next()
                .map(|c| c.to_uppercase().chain(chars).collect())
                .unwrap_or_default();
            format!("{title}: {text}")
        }
    }
}

fn scalar(value: &Value, path: &str) -> Result<Scalar> {
    match value {
        Value::Bool(b) => Ok(Scalar::Bool(*b)),
        Value::String(s) => Ok(Scalar::Str(s.clone())),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Scalar::Int(i))
            } else if let Some(f) = n.as_f64().filter(|f| f.is_finite()) {
                Ok(Scalar::Float(f))
            } else {
                Err(format!(
                    "{path}: number {n} does not fit a 64-bit integer or a finite decimal"
                ))
            }
        }
        Value::Null => Err(format!(
            "{path}: null has no type; give it a value or remove it"
        )),
        Value::Array(_) | Value::Object(_) => Err(format!("{path}: expected a single value here")),
    }
}

fn unify(a: Kind, b: Kind, path: &str) -> Result<Kind> {
    match (a, b) {
        _ if a == b => Ok(a),
        (Kind::Int, Kind::Float) | (Kind::Float, Kind::Int) => Ok(Kind::Float),
        _ => Err(format!(
            "{path}: mixes {a:?} and {b:?} values; a list or record field holds one kind"
        )),
    }
}

fn scalar_list(items: &[Value], path: &str) -> Result<(Kind, Vec<Scalar>)> {
    let values = items
        .iter()
        .enumerate()
        .map(|(i, v)| scalar(v, &format!("{path}[{i}]")))
        .collect::<Result<Vec<_>>>()?;
    let first = values
        .first()
        .ok_or_else(|| format!("{path}: an empty list has no type"))?
        .kind();
    let kind = values
        .iter()
        .try_fold(first, |k, v| unify(k, v.kind(), path))?;
    Ok((kind, values.into_iter().map(|v| v.widen(kind)).collect()))
}

fn parse_list(items: &[Value], path: &str) -> Result<Entry> {
    if items.is_empty() {
        return Err(format!("{path}: an empty list has no type"));
    }
    if items.iter().all(Value::is_object) {
        return parse_records(items, path).map(Entry::Records);
    }
    if items.iter().any(|v| v.is_object() || v.is_array()) {
        return Err(format!(
            "{path}: a list must hold only values or only objects"
        ));
    }
    let (kind, values) = scalar_list(items, path)?;
    Ok(Entry::Field(Field::List(kind, values)))
}

fn parse_records(items: &[Value], path: &str) -> Result<Records> {
    let objects: Vec<&Map<String, Value>> = items.iter().filter_map(Value::as_object).collect();
    let keys: Vec<String> = objects[0]
        .keys()
        .filter(|k| !k.starts_with('$'))
        .cloned()
        .collect();

    // Read every row as raw fields first, then settle each column's type.
    let mut rows: Vec<Vec<Field>> = Vec::new();
    for (i, obj) in objects.iter().enumerate() {
        let here = format!("{path}[{i}]");
        let mut own: Vec<&String> = obj.keys().filter(|k| !k.starts_with('$')).collect();
        own.sort();
        let mut expected: Vec<&String> = keys.iter().collect();
        expected.sort();
        if own != expected {
            return Err(format!(
                "{here}: every object in the list needs the same keys as the first one"
            ));
        }
        let mut row = Vec::new();
        for key in &keys {
            let at = format!("{here}.{key}");
            let field = match &obj[key] {
                Value::Array(list) => {
                    let (kind, values) = scalar_list(list, &at)?;
                    Field::List(kind, values)
                }
                Value::Object(_) => {
                    return Err(format!("{at}: objects inside list items are not supported"));
                }
                other => Field::One(scalar(other, &at)?),
            };
            row.push(field);
        }
        rows.push(row);
    }

    let mut fields = Vec::new();
    for (col, key) in keys.iter().enumerate() {
        let at = format!("{path}[*].{key}");
        let mut ty: Option<FieldType> = None;
        for row in &rows {
            let this = match &row[col] {
                Field::One(s) => FieldType::One(s.kind()),
                Field::List(k, _) => FieldType::List(*k),
            };
            ty = Some(match (ty, this) {
                (None, t) => t,
                (Some(FieldType::One(a)), FieldType::One(b)) => FieldType::One(unify(a, b, &at)?),
                (Some(FieldType::List(a)), FieldType::List(b)) => {
                    FieldType::List(unify(a, b, &at)?)
                }
                _ => {
                    return Err(format!(
                        "{at}: is a list in some items and a single value in others"
                    ));
                }
            });
        }
        let ty = ty.expect("records have at least one row");
        for row in &mut rows {
            row[col] = match (
                std::mem::replace(&mut row[col], Field::One(Scalar::Bool(false))),
                ty,
            ) {
                (Field::One(s), FieldType::One(k)) => Field::One(s.widen(k)),
                (Field::List(_, v), FieldType::List(k)) => {
                    Field::List(k, v.into_iter().map(|s| s.widen(k)).collect())
                }
                (other, _) => other,
            };
        }
        fields.push((key.clone(), ty));
    }
    Ok(Records { fields, rows })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dollar_keys_become_docs() {
        let g =
            parse(r#"{"$source":"board 4","$description":"Hello","$units":"pt","a":1}"#).unwrap();
        assert_eq!(g.docs, vec!["Source: board 4", "Hello", "Units: pt"]);
        assert_eq!(g.entries.len(), 1);
    }

    #[test]
    fn keeps_the_order_of_tokens_json() {
        let g = parse(r#"{"zeta":1,"alpha":2,"mid":3}"#).unwrap();
        let keys: Vec<_> = g.entries.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["zeta", "alpha", "mid"]);
    }

    #[test]
    fn whole_numbers_and_decimals_mix_into_decimals() {
        let g = parse(r#"{"a":[1,0.5,0.25]}"#).unwrap();
        assert_eq!(
            g.entries[0].1,
            Entry::Field(Field::List(
                Kind::Float,
                vec![Scalar::Float(1.0), Scalar::Float(0.5), Scalar::Float(0.25)]
            ))
        );
    }

    #[test]
    fn other_mixes_are_errors() {
        let err = parse(r#"{"a":{"b":[1,"x"]}}"#).unwrap_err();
        assert!(err.contains("tokens.a.b"), "{err}");
    }

    #[test]
    fn null_and_empty_lists_are_errors() {
        assert!(parse(r#"{"a":null}"#).unwrap_err().contains("null"));
        assert!(parse(r#"{"a":[]}"#).unwrap_err().contains("empty list"));
    }

    #[test]
    fn records_need_the_same_keys() {
        assert!(
            parse(r#"{"a":[{"x":1},{"y":2}]}"#)
                .unwrap_err()
                .contains("same keys")
        );
        assert!(parse(r#"{"a":[{"x":1,"y":2},{"y":3,"x":4}]}"#).is_ok());
    }

    #[test]
    fn record_columns_widen_together() {
        let g = parse(r#"{"a":[{"x":1},{"x":2.5}]}"#).unwrap();
        let Entry::Records(r) = &g.entries[0].1 else {
            panic!("expected records")
        };
        assert_eq!(
            r.fields,
            vec![("x".to_string(), FieldType::One(Kind::Float))]
        );
        assert_eq!(r.rows[0], vec![Field::One(Scalar::Float(1.0))]);
    }

    #[test]
    fn lists_of_lists_are_errors() {
        assert!(parse(r#"{"a":[[1],[2]]}"#).is_err());
    }

    #[test]
    fn groups_that_would_hide_a_built_in_type_are_errors() {
        let err = parse(r#"{"a":{"string":{"x":1}}}"#).unwrap_err();
        assert!(err.contains("tokens.a.string"), "{err}");
        assert!(parse(r#"{"a":{"strings":{"x":1}}}"#).is_ok());
        // A plain value with that name is fine: members are camelCase.
        assert!(parse(r#"{"a":{"string":1}}"#).is_ok());
    }

    #[test]
    fn the_real_tokens_file_parses() {
        let json = include_str!("../../../design/tokens.json");
        let g = parse(json).unwrap();
        assert!(g.entries.iter().any(|(k, _)| k == "breakpoints"));
        assert!(g.entries.iter().any(|(k, _)| k == "recap"));
    }
}
