//! %Dictionary introspection tools for dynamic dispatch resolution.

use crate::iris::connection::IrisConnection;
use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::HashMap;

pub const METADATA_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(60);
pub type MetadataCache = std::sync::Mutex<HashMap<String, (serde_json::Value, std::time::Instant)>>;

pub fn metadata_cache_get(cache: &MetadataCache, key: &str) -> Option<serde_json::Value> {
    let map = cache.lock().unwrap();
    map.get(key).and_then(|(v, ts)| {
        if ts.elapsed() < METADATA_CACHE_TTL {
            Some(v.clone())
        } else {
            None
        }
    })
}

pub fn metadata_cache_set(cache: &MetadataCache, key: String, val: serde_json::Value) {
    cache
        .lock()
        .unwrap()
        .insert(key, (val, std::time::Instant::now()));
}

pub fn confidence_for_count(n: usize) -> f64 {
    match n {
        0 => 0.0,
        1 => 0.90,
        2..=5 => 0.75,
        6..=20 => 0.55,
        _ => 0.30,
    }
}

fn ok_json(v: serde_json::Value) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    Ok(rmcp::model::CallToolResult::success(vec![
        rmcp::model::Content::text(v.to_string()),
    ]))
}

fn err_json(code: &str, msg: &str) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    ok_json(serde_json::json!({"success": false, "error_code": code, "error": msg}))
}

fn default_namespace() -> String {
    "USER".to_string()
}

// ── Tool 1: resolve_dynamic_dispatch ─────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ResolveDynamicDispatchParams {
    /// Method name to search for (e.g. "ExecuteQuery", "Connect", "OnProcessInput").
    pub method_name: String,
    /// Optional package prefix to restrict search (e.g. "EnsLib", "HS").
    pub package_prefix: Option<String>,
    /// IRIS namespace. Defaults to "USER".
    #[serde(default = "default_namespace")]
    pub namespace: String,
    /// Max candidates to return. Defaults to 50.
    pub limit: Option<usize>,
}

pub async fn handle_resolve_dynamic_dispatch(
    iris: &IrisConnection,
    client: &reqwest::Client,
    p: ResolveDynamicDispatchParams,
    cache: &MetadataCache,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    let prefix = p.package_prefix.as_deref().unwrap_or("");
    let limit = p.limit.unwrap_or(50);
    let cache_key = format!(
        "resolve_dynamic_dispatch:{}:{}:{}",
        p.method_name, prefix, p.namespace
    );
    if let Some(cached) = metadata_cache_get(cache, &cache_key) {
        return ok_json(cached);
    }

    let has_prefix = !prefix.is_empty();
    let method_esc = p.method_name.replace('"', "\\\"");
    let prefix_esc = prefix.replace('"', "\\\"");

    // Build ObjectScript — use q=$CHAR(34) for embedded JSON quotes
    let mut lines: Vec<String> = vec!["Set q=$CHAR(34)".into()];
    let mut sql = "SELECT m.parent, m.Origin, m.FormalSpec FROM %Dictionary.CompiledMethod m WHERE m.Name = ? AND m.Origin = m.parent".to_string();
    if has_prefix {
        sql.push_str(" AND m.parent %STARTSWITH ?");
    }
    sql.push_str(&format!(
        " ORDER BY m.parent FETCH FIRST {} ROWS ONLY",
        limit
    ));
    lines.push(format!(r#"Set sql="{}""#, sql));
    if has_prefix {
        lines.push(format!(
            r#"Set rs=##class(%SQL.Statement).%ExecDirect(,sql,"{}","{}")"#,
            method_esc, prefix_esc
        ));
    } else {
        lines.push(format!(
            r#"Set rs=##class(%SQL.Statement).%ExecDirect(,sql,"{}")"#,
            method_esc
        ));
    }
    lines.push(r#"If rs.%SQLCODE<0 { Write "ERROR:"_rs.%Message,! Quit }"#.into());
    lines.push(r#"Set out="[",sep="""#.into());
    lines.push("While rs.%Next() {".into());
    lines.push(r#"  Set out=out_sep_"{"_q_"class"_q_":"_q_rs.parent_q_","_q_"origin"_q_":"_q_rs.Origin_q_","_q_"formal_spec"_q_":"_q_rs.FormalSpec_q_"}""#.into());
    lines.push(r#"  Set sep=",""#.into());
    lines.push("}".into());
    lines.push(r#"Write out_"]",!"#.into());
    let code = lines.join("\n");

    let output = iris
        .execute_via_generator(&code, &p.namespace, client)
        .await
        .map_err(|e| rmcp::ErrorData::internal_error(format!("execute failed: {e}"), None))?;
    let trimmed = output.trim();

    if trimmed.is_empty() {
        let r = serde_json::json!({"success":true,"method_name":p.method_name,"candidates":[],"candidate_count":0,"confidence":0.0,"truncated":false});
        metadata_cache_set(cache, cache_key, r.clone());
        return ok_json(r);
    }
    if let Some(msg) = trimmed.strip_prefix("ERROR:") {
        return err_json("QUERY_ERROR", msg.trim());
    }

    let raw: serde_json::Value = serde_json::from_str(trimmed).unwrap_or(serde_json::json!([]));
    let candidates = raw.as_array().cloned().unwrap_or_default();
    let n = candidates.len();
    let confidence = confidence_for_count(n);
    let annotated: Vec<_> = candidates
        .into_iter()
        .map(|mut c| {
            c["confidence"] = serde_json::json!(confidence);
            c
        })
        .collect();
    let r = serde_json::json!({"success":true,"method_name":p.method_name,"package_prefix":p.package_prefix,"namespace":p.namespace,"candidates":annotated,"candidate_count":n,"confidence":confidence,"truncated":n==limit});
    metadata_cache_set(cache, cache_key, r.clone());
    ok_json(r)
}

// ── Tool 2: extract_message_map_routing ───────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExtractMessageMapParams {
    /// Fully qualified Ensemble class name (e.g. "HS.Flash.Router").
    pub class_name: String,
    /// IRIS namespace. Defaults to "USER".
    #[serde(default = "default_namespace")]
    pub namespace: String,
}

fn build_message_map_code(cls: &str) -> String {
    let cls_esc = cls.replace('\\', "\\\\").replace('"', "\\\"");
    let mut lines: Vec<String> = vec!["Set q=$CHAR(34)".into()];
    lines.push(format!(
        r#"Set clsObj=##class(%Dictionary.CompiledClass).%OpenId("{}")"#,
        cls_esc
    ));
    lines.push(r#"If '$IsObject(clsObj) { Write "NOT_FOUND",! Quit }"#.into());
    lines.push(format!(
        r#"Set xd=##class(%Dictionary.CompiledXData).%OpenId("{}||MessageMap")"#,
        cls_esc
    ));
    lines.push(r#"If '$IsObject(xd) { Write "{"_q_"has_message_map"_q_":false,"_q_"routes"_q_":[]}", ! Quit }"#.into());
    lines.push("Do xd.Data.Rewind()".into());
    lines.push("Set reader=##class(%XML.TextReader).%New()".into());
    lines.push("Set sc=reader.ParseStream(xd.Data)".into());
    lines.push(
        r#"If $$$ISERR(sc) { Write "{"_q_"error"_q_":"_q_"parse_failed"_q_"}", ! Quit }"#.into(),
    );
    lines.push(r#"Set routes="[",sep="",msgType="",inMapItem=0"#.into());
    lines.push("While reader.Read() {".into());
    lines.push(r#"  If reader.NodeType="element" {"#.into());
    lines.push(r#"    If reader.Name="MapItem" { Set msgType=reader.GetAttributeValue("MessageType"),inMapItem=1 }"#.into());
    lines.push(r#"    ElseIf (reader.Name="Method")&&inMapItem { Do reader.Read()"#.into());
    lines.push(r#"      If reader.NodeType="chars" {"#.into());
    lines.push(r#"        Set routes=routes_sep_"{"_q_"message_type"_q_":"_q_msgType_q_","_q_"method"_q_":"_q_reader.Value_q_","_q_"confidence"_q_":0.9}""#.into());
    lines.push(r#"        Set sep=",""#.into());
    lines.push("      }".into());
    lines.push("    }".into());
    lines.push(r#"  } ElseIf (reader.NodeType="endelement")&&(reader.Name="MapItem") { Set inMapItem=0,msgType="" }"#.into());
    lines.push("}".into());
    lines.push(r#"Set routes=routes_"]""#.into());
    lines.push(r#"Write "{"_q_"has_message_map"_q_":true,"_q_"routes"_q_":"_routes_"}",!"#.into());
    lines.join("\n")
}

pub async fn handle_extract_message_map_routing(
    iris: &IrisConnection,
    client: &reqwest::Client,
    p: ExtractMessageMapParams,
    cache: &MetadataCache,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    let cache_key = format!("extract_message_map:{}:{}", p.class_name, p.namespace);
    if let Some(cached) = metadata_cache_get(cache, &cache_key) {
        return ok_json(cached);
    }

    let code = build_message_map_code(&p.class_name);
    let output = iris
        .execute_via_generator(&code, &p.namespace, client)
        .await
        .map_err(|e| rmcp::ErrorData::internal_error(format!("execute failed: {e}"), None))?;
    let trimmed = output.trim();

    if trimmed == "NOT_FOUND" {
        return err_json("NOT_FOUND", &format!("Class '{}' not found", p.class_name));
    }

    let inner: serde_json::Value = serde_json::from_str(trimmed).map_err(|e| {
        rmcp::ErrorData::internal_error(format!("parse failed: {e} raw={trimmed}"), None)
    })?;
    if let Some(err) = inner.get("error") {
        return err_json("PARSE_ERROR", err.as_str().unwrap_or("xml parse failed"));
    }

    let has_mm = inner["has_message_map"].as_bool().unwrap_or(false);
    let routes = inner["routes"].as_array().cloned().unwrap_or_default();
    let route_count = routes.len();
    let r = serde_json::json!({"success":true,"class_name":p.class_name,"namespace":p.namespace,"has_message_map":has_mm,"routes":routes,"route_count":route_count});
    metadata_cache_set(cache, cache_key, r.clone());
    ok_json(r)
}

// ── Tool 3: find_subclass_implementations ────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindSubclassImplementationsParams {
    /// Method name (e.g. "OnProcessInput", "Execute").
    pub method_name: String,
    /// Base classes to expand — all descendants searched (e.g. ["Ens.BusinessProcess"]).
    pub base_classes: Vec<String>,
    /// IRIS namespace. Defaults to "USER".
    #[serde(default = "default_namespace")]
    pub namespace: String,
    /// Max results. Defaults to 100.
    pub limit: Option<usize>,
}

fn build_expand_hierarchy_code(base_classes: &[String]) -> String {
    let bases_list = base_classes
        .iter()
        .map(|c| format!(r#"$LISTBUILD("{}")"#, c.replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join("_");
    format!(
        r#"
Set queue={bases}
Set result=queue
While $LISTLENGTH(queue)>0 {{
    Set base=$LISTGET(queue,1),queue=$LIST(queue,2,*)
    Set rs2=##class(%SQL.Statement).%ExecDirect(,"SELECT Name FROM %Dictionary.CompiledClass WHERE Super LIKE ?","%"_base_"%")
    While rs2.%Next() {{
        Set child=rs2.Name
        If '$LISTFIND(result,child) {{ Set result=result_$LISTBUILD(child),queue=queue_$LISTBUILD(child) }}
    }}
}}
Set out="",sep=""
For i=1:1:$LISTLENGTH(result) {{ Set out=out_sep_$LISTGET(result,i),sep="|" }}
Write out,!
"#,
        bases = bases_list
    )
}

pub async fn handle_find_subclass_implementations(
    iris: &IrisConnection,
    client: &reqwest::Client,
    p: FindSubclassImplementationsParams,
    cache: &MetadataCache,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    if p.base_classes.is_empty() {
        return err_json("INVALID_PARAMS", "base_classes must not be empty");
    }

    let mut sorted = p.base_classes.clone();
    sorted.sort();
    let cache_key = format!(
        "find_subclass:{}:{}:{}",
        p.method_name,
        sorted.join(","),
        p.namespace
    );
    if let Some(cached) = metadata_cache_get(cache, &cache_key) {
        return ok_json(cached);
    }

    let limit = p.limit.unwrap_or(100);
    let expand_code = build_expand_hierarchy_code(&p.base_classes);
    let desc_raw = iris
        .execute_via_generator(&expand_code, &p.namespace, client)
        .await
        .map_err(|e| {
            rmcp::ErrorData::internal_error(format!("hierarchy expansion failed: {e}"), None)
        })?;

    let descendants: Vec<String> = desc_raw
        .trim()
        .split('|')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    if descendants.is_empty() {
        let r = serde_json::json!({"success":true,"method_name":p.method_name,"base_classes":p.base_classes,"namespace":p.namespace,"implementations":[],"implementation_count":0,"confidence":0.0});
        metadata_cache_set(cache, cache_key, r.clone());
        return ok_json(r);
    }

    let method_esc = p.method_name.replace('"', "\\\"");
    let desc_list = descendants
        .iter()
        .map(|c| format!(r#"$LISTBUILD("{}")"#, c.replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join("_");

    let mut lines: Vec<String> = vec!["Set q=$CHAR(34)".into()];
    lines.push(format!("Set descList={}", desc_list));
    lines.push(format!(r#"Set rs=##class(%SQL.Statement).%ExecDirect(,"SELECT m.parent, m.FormalSpec FROM %Dictionary.CompiledMethod m WHERE m.Name = ? AND m.Origin = m.parent ORDER BY m.parent FETCH FIRST {} ROWS ONLY","{}")"#, limit, method_esc));
    lines.push(r#"If rs.%SQLCODE<0 { Write "ERROR:"_rs.%Message,! Quit }"#.into());
    lines.push(r#"Set out="[",sep="""#.into());
    lines.push("While rs.%Next() {".into());
    lines.push(r#"  If $LISTFIND(descList,rs.parent) {"#.into());
    lines.push(r#"    Set out=out_sep_"{"_q_"class"_q_":"_q_rs.parent_q_","_q_"formal_spec"_q_":"_q_rs.FormalSpec_q_"}""#.into());
    lines.push(r#"    Set sep=",""#.into());
    lines.push("  }".into());
    lines.push("}".into());
    lines.push(r#"Write out_"]",!"#.into());
    let code = lines.join("\n");

    let output = iris
        .execute_via_generator(&code, &p.namespace, client)
        .await
        .map_err(|e| rmcp::ErrorData::internal_error(format!("method query failed: {e}"), None))?;
    let trimmed = output.trim();

    if let Some(msg) = trimmed.strip_prefix("ERROR:") {
        return err_json("QUERY_ERROR", msg.trim());
    }
    let raw: serde_json::Value = serde_json::from_str(trimmed).unwrap_or(serde_json::json!([]));
    let impls = raw.as_array().cloned().unwrap_or_default();
    let n = impls.len();
    let confidence = confidence_for_count(n);
    let annotated: Vec<_> = impls
        .into_iter()
        .map(|mut i| {
            i["confidence"] = serde_json::json!(confidence);
            i
        })
        .collect();
    let r = serde_json::json!({"success":true,"method_name":p.method_name,"base_classes":p.base_classes,"namespace":p.namespace,"implementations":annotated,"implementation_count":n,"confidence":confidence});
    metadata_cache_set(cache, cache_key, r.clone());
    ok_json(r)
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_hit_returns_value() {
        let cache: MetadataCache = std::sync::Mutex::new(HashMap::new());
        let val = serde_json::json!({"x": 1});
        metadata_cache_set(&cache, "k".into(), val.clone());
        assert_eq!(metadata_cache_get(&cache, "k"), Some(val));
    }

    #[test]
    fn test_cache_miss_after_ttl() {
        let cache: MetadataCache = std::sync::Mutex::new(HashMap::new());
        let val = serde_json::json!({"x": 1});
        cache.lock().unwrap().insert(
            "k".into(),
            (
                val,
                std::time::Instant::now() - std::time::Duration::from_secs(120),
            ),
        );
        assert_eq!(metadata_cache_get(&cache, "k"), None);
    }

    #[test]
    fn test_cache_set_overwrites() {
        let cache: MetadataCache = std::sync::Mutex::new(HashMap::new());
        metadata_cache_set(&cache, "k".into(), serde_json::json!(1));
        metadata_cache_set(&cache, "k".into(), serde_json::json!(2));
        assert_eq!(metadata_cache_get(&cache, "k"), Some(serde_json::json!(2)));
    }

    #[test]
    fn test_confidence_formula() {
        assert_eq!(confidence_for_count(0), 0.0);
        assert_eq!(confidence_for_count(1), 0.90);
        assert_eq!(confidence_for_count(3), 0.75);
        assert_eq!(confidence_for_count(10), 0.55);
        assert_eq!(confidence_for_count(25), 0.30);
    }

    #[test]
    fn test_no_message_map_json_shape() {
        let s = r#"{"has_message_map":false,"routes":[]}"#;
        let v: serde_json::Value = serde_json::from_str(s).unwrap();
        assert_eq!(v["has_message_map"], false);
        assert!(v["routes"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_expand_hierarchy_code_nonempty() {
        let code = build_expand_hierarchy_code(&["Ens.BusinessProcess".to_string()]);
        assert!(code.contains("Ens.BusinessProcess"));
        assert!(code.contains("LIKE"));
    }
}

#[cfg(test)]
mod query_debug_tests {
    use super::*;
    #[test]
    fn print_find_subclass_query_code() {
        let descendants = vec!["Ens.BusinessProcess".to_string()];
        let method_esc = "OnProcessInput";
        let limit: usize = 100;
        let desc_list = descendants
            .iter()
            .map(|c| format!(r#"$LISTBUILD("{}")"#, c.replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join("_");
        let mut lines: Vec<String> = vec!["Set q=$CHAR(34)".into()];
        lines.push(format!("Set descList={}", desc_list));
        lines.push(format!(r#"Set rs=##class(%SQL.Statement).%ExecDirect(,"SELECT m.parent, m.FormalSpec FROM %Dictionary.CompiledMethod m WHERE m.Name = ? AND m.Origin = m.parent ORDER BY m.parent FETCH FIRST {} ROWS ONLY","{}")"#, limit, method_esc));
        let code = lines.join("\n");
        println!("GENERATED CODE:\n{}", code);
    }
}
