use std::sync::Arc;

use super::*;

fn codes_of(text: &str, schema: &Schema) -> Vec<&'static str> {
    match load(text, schema) {
        Ok(_) => Vec::new(),
        Err(diags) => diags.iter().map(|d| d.code.id).collect(),
    }
}

fn server() -> Schema {
    Schema::Struct {
        id: 0,
        name: "Server".into(),
        fields: vec![
            ("port".into(), Arc::new(Schema::Int)),
            ("name".into(), Arc::new(Schema::String)),
            (
                "mode".into(),
                Arc::new(Schema::Enum {
                    id: 0,
                    name: "Mode".into(),
                    variants: vec!["Debug".into(), "Release".into()],
                }),
            ),
            ("ratio".into(), Arc::new(Schema::Float)),
            (
                "hosts".into(),
                Arc::new(Schema::Array(Arc::new(Schema::String))),
            ),
            (
                "limits".into(),
                Arc::new(Schema::Map(Arc::new(Schema::Int))),
            ),
        ],
    }
}

const GOOD: &str = r#"
// configuration de test
{
  "port": 8080, /* port d'écoute */
  "name": "prod \u00e9\ud83d\ude00",
  "mode": "Release",
  "ratio": 2,
  "hosts": ["a", "b"],
  "limits": { "users": 10 }
}
"#;

#[test]
fn accepts_valid_file_with_comments() {
    assert_eq!(codes_of(GOOD, &server()), Vec::<&str>::new());
    let v = parse(GOOD).unwrap();
    let JsonKind::Object(m) = &v.kind else {
        panic!()
    };
    assert_eq!(m[1].value.kind, JsonKind::Str("prod é😀".into()));
}

#[test]
fn syntax_errors() {
    let s = Schema::Int;
    for bad in [
        "",
        "{",
        "[1, 2,]",
        "{\"a\": 1,}",
        "{a: 1}",
        "'x'",
        "01",
        "1.",
        "\"a\nb\"",
        "/* jamais fermé",
        "1 2",
        "True",
        r#""\x""#,
        r#""\ud800""#,
    ] {
        assert_eq!(codes_of(bad, &s), vec!["MCF0600"], "{bad:?}");
    }
    assert_eq!(codes_of("{\"a\": 1, \"a\": 2}", &s), vec!["MCF0601"]);
    let deep = "[".repeat(MAX_DEPTH + 1) + &"]".repeat(MAX_DEPTH + 1);
    assert_eq!(codes_of(&deep, &s), vec!["MCF0602"]);
}

#[test]
fn type_errors_are_all_reported() {
    let text = r#"{
      "port": 80.5,
      "name": null,
      "mode": "release",
      "ratio": 1e999,
      "hosts": ["a", 3],
      "limits": { "users": "x" },
      "prot": 1
    }"#;
    assert_eq!(
        codes_of(text, &server()),
        vec!["MCF0620", "MCF0620", "MCF0623", "MCF0624", "MCF0620", "MCF0620", "MCF0622"]
    );
    assert_eq!(codes_of("{}", &server()).len(), 6);
    assert_eq!(
        codes_of("9223372036854775808", &Schema::Int),
        vec!["MCF0624"]
    );
    assert_eq!(
        codes_of("-9223372036854775808", &Schema::Int),
        Vec::<&str>::new()
    );
}

#[test]
fn suggestions() {
    let Err(d) = load(
        r#"{"port": 1, "nme": "x", "mode": "debug", "ratio": 1, "hosts": [], "limits": {}}"#,
        &server(),
    ) else {
        panic!()
    };
    assert_eq!(
        d[0].suggestion.as_deref(),
        Some("vouliez-vous dire \"name\" ?")
    );
    assert_eq!(
        d[1].suggestion.as_deref(),
        Some("vouliez-vous dire \"Debug\" ?")
    );
}

#[test]
fn dual_types() {
    let num = Schema::Dual(Arc::new(Schema::Int), Arc::new(Schema::Float));
    let v = parse("3").unwrap();
    assert_eq!(
        pick_member(&v, &Schema::Int, &Schema::Float),
        Some(&Schema::Int)
    );
    let v = parse("3.5").unwrap();
    assert_eq!(
        pick_member(&v, &Schema::Int, &Schema::Float),
        Some(&Schema::Float)
    );
    assert!(codes_of("3.5", &num).is_empty());
    let is = Schema::Dual(Arc::new(Schema::Int), Arc::new(Schema::String));
    assert!(codes_of("\"x\"", &is).is_empty());
    assert_eq!(codes_of("true", &is), vec!["MCF0620"]);
    assert!(ambiguous_dual(
        &Schema::String,
        &Schema::Enum {
            id: 0,
            name: "E".into(),
            variants: vec![]
        }
    ));
    assert!(!ambiguous_dual(&Schema::Int, &Schema::Float));
}

#[test]
fn paths() {
    assert!(check_path("a/b.cfg").is_ok());
    assert!(check_path("../shared/b.cfg").is_ok());
    for bad in ["", r"a\b.cfg", "/a.cfg", "C:/a.cfg", "a.json"] {
        assert!(check_path(bad).is_err(), "{bad}");
    }
    assert_eq!(
        resolve_path("proj/src/main.mcf", "../cfg/./s.cfg").as_deref(),
        Some("proj/cfg/s.cfg")
    );
    assert_eq!(resolve_path("main.mcf", "s.cfg").as_deref(), Some("s.cfg"));
    assert_eq!(normalize_name(r"proj\src\..\cfg\s.cfg"), "proj/cfg/s.cfg");
}
