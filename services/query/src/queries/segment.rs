use std::collections::HashSet;

use crate::ch::client::{QueryParam, QueryPlan};
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

pub fn build_plan_condition(
    cond: &SegmentCondition,
    project_id: &str,
    limit: u64,
) -> Result<QueryPlan, String> {
    let op = sql_op(&cond.op)?;

    match cond.condition_type.as_str() {
        "event_count" => {
            let event_name = cond
                .event_name
                .as_deref()
                .ok_or("event_count condition requires event_name")?;
            let within = cond.within_days.unwrap_or(30);

            let sql = format!(
                "SELECT COALESCE(user_id, anonymous_id) AS user_key
FROM events
WHERE project_id = ?
  AND event_name = ?
  AND timestamp >= now() - INTERVAL ? DAY
GROUP BY user_key
HAVING count() {} ?
LIMIT ?",
                op,
            );

            let params = vec![
                QueryParam::String(project_id.to_string()),
                QueryParam::String(event_name.to_string()),
                QueryParam::U64(within),
                value_to_param(&cond.value),
                QueryParam::U64(limit),
            ];

            Ok(QueryPlan::new(sql, params))
        }
        "trait" => {
            let key = cond.key.as_deref().ok_or("trait condition requires key")?;

            let (op_sql, value_param) = if cond.op == "contains" {
                let pattern = format!(
                    "%{}%",
                    like_escape(cond.value.to_string().trim_matches('"'))
                );
                ("LIKE", QueryParam::String(pattern))
            } else {
                (op, value_to_param(&cond.value))
            };

            let sql = format!(
                "SELECT DISTINCT COALESCE(user_id, anonymous_id) AS user_key
FROM events
WHERE project_id = ?
  AND event_name = 'Identify'
  AND properties[?] {} ?
LIMIT ?",
                op_sql,
            );

            let params = vec![
                QueryParam::String(project_id.to_string()),
                QueryParam::String(key.to_string()),
                value_param,
                QueryParam::U64(limit),
            ];

            Ok(QueryPlan::new(sql, params))
        }
        _ => Err(format!(
            "unsupported condition type: {}",
            cond.condition_type
        )),
    }
}

pub fn build_plan(spec: &SegmentSpec) -> Result<(Vec<QueryPlan>, u64), String> {
    let subquery_limit = spec.limit.saturating_add(1);
    if spec.conditions.is_empty() {
        let sql = format!(
            "SELECT DISTINCT COALESCE(user_id, anonymous_id) AS user_key
FROM events
WHERE project_id = ?
LIMIT ?"
        );
        let plan = QueryPlan::new(
            sql,
            vec![
                QueryParam::String(spec.project_id.clone()),
                QueryParam::U64(subquery_limit),
            ],
        );
        return Ok((vec![plan], spec.limit));
    }
    let mut plans = Vec::new();
    for cond in &spec.conditions {
        let plan = build_plan_condition(cond, &spec.project_id, subquery_limit)?;
        plans.push(plan);
    }
    Ok((plans, spec.limit))
}

fn value_to_param(val: &serde_json::Value) -> QueryParam {
    match val {
        serde_json::Value::String(s) => QueryParam::String(s.clone()),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                QueryParam::I64(i)
            } else {
                QueryParam::F64(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::Bool(b) => QueryParam::String(b.to_string()),
        _ => QueryParam::String(val.to_string()),
    }
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

fn like_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}
