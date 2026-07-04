use std::collections::HashSet;

use crate::{SegmentCondition, SegmentSpec};

fn sql_op(op: &str) -> Result<&'static str, String> {
    match op {
        "eq" => Ok("="),
        "neq" => Ok("!="),
        "gt" => Ok(">"),
        "gte" => Ok(">="),
        "lt" => Ok("<"),
        "lte" => Ok("<="),
        "contains" => Ok("LIKE"),
        _ => Err(format!("unsupported operator: {}", op)),
    }
}

fn sql_value(val: &serde_json::Value, op: &str) -> String {
    if op == "contains" {
        format!("'%{}%'", sql_escape(val.to_string().trim_matches('"')))
    } else {
        match val {
            serde_json::Value::String(s) => format!("'{}'", sql_escape(s)),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => if *b { "'true'" } else { "'false'" }.to_string(),
            _ => format!("'{}'", sql_escape(&val.to_string())),
        }
    }
}

pub fn build_condition(
    cond: &SegmentCondition,
    project_id: &str,
    limit: u64,
) -> Result<String, String> {
    let op = sql_op(&cond.op)?;
    let value_sql = sql_value(&cond.value, &cond.op);

    match cond.condition_type.as_str() {
        "event_count" => {
            let event_name = cond
                .event_name
                .as_deref()
                .ok_or("event_count condition requires event_name")?;
            let within = cond.within_days.unwrap_or(30);

            Ok(format!(
                "SELECT COALESCE(user_id, anonymous_id) AS user_key
FROM events
WHERE project_id = '{}'
  AND event_name = '{}'
  AND timestamp >= now() - INTERVAL {} DAY
GROUP BY user_key
HAVING count() {} {}
LIMIT {}",
                sql_escape(project_id),
                sql_escape(event_name),
                within,
                op,
                value_sql,
                limit,
            ))
        }
        "trait" => {
            let key = cond.key.as_deref().ok_or("trait condition requires key")?;
            Ok(format!(
                "SELECT DISTINCT COALESCE(user_id, anonymous_id) AS user_key
FROM events
WHERE project_id = '{}'
  AND event_name = 'Identify'
  AND properties['{}'] {} {}
LIMIT {}",
                sql_escape(project_id),
                sql_escape(key),
                op,
                value_sql,
                limit,
            ))
        }
        _ => Err(format!(
            "unsupported condition type: {}",
            cond.condition_type
        )),
    }
}

pub fn build(spec: &SegmentSpec) -> Result<(Vec<String>, u64), String> {
    let mut subqueries = Vec::new();
    for cond in &spec.conditions {
        let sql = build_condition(cond, &spec.project_id, spec.limit)?;
        subqueries.push(sql);
    }
    Ok((subqueries, spec.limit))
}

pub fn intersect(results: Vec<HashSet<String>>) -> HashSet<String> {
    if results.is_empty() {
        return HashSet::new();
    }
    let mut intersection = results[0].clone();
    for set in &results[1..] {
        intersection.retain(|key| set.contains(key));
    }
    intersection
}

fn sql_escape(s: &str) -> String {
    s.replace('\'', "\\'")
}
