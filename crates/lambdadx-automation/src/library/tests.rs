use super::*;
use crate::{bridge, catalog};
use serde_json::{Value, json};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "ldx-library-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn index(&self, revision: &str, entries: Vec<Value>) {
        fs::write(
            self.0.join("catalog.json"),
            serde_json::to_vec(&json!({"schema_version":1,"revision":revision,"entries":entries}))
                .unwrap(),
        )
        .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn entry(id: &str) -> Value {
    json!({"id":id,"name":"交互","aliases":["定轨交互"],"tags":["TAP"],"summary":"匀拍左右手交替","completeness":"complete","source":"fixture","body":{"path":"body.md","bytes":12},"references":[{"id":"sample","label":"参考谱","kind":"simai","file":{"path":"sample.simai","bytes":1024*1024*1024}}]})
}
fn search(query: &str) -> LibrarySearchRequest {
    LibrarySearchRequest {
        schema_version: 1,
        query: query.into(),
        offset: 0,
        limit: 5,
        revision: None,
    }
}
fn read(revision: &str) -> LibraryReadRequest {
    LibraryReadRequest {
        schema_version: 1,
        revision: revision.into(),
        id: "interaction".into(),
        part: None,
        offset: 0,
        max_bytes: 4,
        file_revision: None,
    }
}
fn bundled() -> Library {
    Library::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../maimai-note-desinger-skill"))
}
fn resolve(prompt: &str) -> LibraryResolveRequest {
    LibraryResolveRequest {
        schema_version: 1,
        prompt: prompt.into(),
        previous_prompts: vec![],
    }
}

#[test]
fn resolves_specific_names_aliases_and_followups_without_unrelated_definitions() {
    let mut lib = bundled();
    for (prompt, id) in [
        ("生成32个三角交互", "triangle"),
        ("来一段方向盘交互", "reverse-staircase"),
        ("生成 mixed-rhythm", "mixed-rhythm"),
    ] {
        let result = lib.resolve(&resolve(prompt)).unwrap();
        assert_eq!(result.definitions.len(), 1);
        assert_eq!(result.definitions[0].id, id);
        assert!(!result.definitions[0].text.is_empty());
    }
    let result = lib.resolve(&resolve("生成三角交互，然后交互")).unwrap();
    assert_eq!(result.definitions.len(), 2);
    assert!(
        lib.resolve(&resolve("生成 jacket notes"))
            .unwrap()
            .definitions
            .is_empty()
    );
    let mut request = resolve("增加到32个");
    request.previous_prompts = vec!["来一段轴三角".into(), "生成纵连".into()];
    assert_eq!(
        lib.resolve(&request).unwrap().definitions[0].id,
        "anchor-triangle"
    );
    request.prompt = "换成散点".into();
    assert_eq!(
        lib.resolve(&request).unwrap().definitions[0].id,
        "scattered"
    );
}

#[test]
fn resolves_complete_paged_definitions_and_rejects_excess_instead_of_truncating() {
    let fixture = Fixture::new();
    let body = "中文配置".repeat(1000);
    fs::write(fixture.0.join("body.md"), &body).unwrap();
    let mut e = entry("interaction");
    e["body"]["bytes"] = json!(body.len());
    fixture.index("paged", vec![e.clone()]);
    let mut lib = Library::new(fixture.0.clone());
    assert_eq!(
        lib.resolve(&resolve("交互")).unwrap().definitions[0].text,
        body
    );
    // The reference file does not exist; resolving definitions must never open it.
    e["body"]["bytes"] = json!(MAX_DEFINITION_BYTES + 1);
    fixture.index("too-large", vec![e.clone()]);
    assert!(
        lib.resolve(&resolve("交互"))
            .unwrap_err()
            .message
            .contains("48 KiB")
    );
    fixture.index(
        "too-many",
        (0..9)
            .map(|i| {
                let mut e = e.clone();
                e["id"] = json!(format!("pattern-{i}"));
                e
            })
            .collect(),
    );
    assert!(
        lib.resolve(&resolve("交互"))
            .unwrap_err()
            .message
            .contains("8 configurations")
    );
    assert!(
        catalog::parse(
            "library_resolve",
            json!({"schema_version":1,"prompt":"x","extra":true})
        )
        .is_err()
    );
}

#[test]
fn resolve_scales_to_ten_thousand_metadata_entries_without_opening_unmatched_files() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("body.md"), "中文测试").unwrap();
    fixture.index(
        "large-resolve",
        (0..10_000)
            .map(|i| {
                let mut e = entry(&format!("pattern-{i:05}"));
                e["name"] = json!(format!("配置{i}"));
                e["aliases"] = json!([format!("别名{i}")]);
                if i != 9999 {
                    e["body"]["path"] = json!("does-not-exist.md");
                }
                e
            })
            .collect(),
    );
    let mut lib = Library::new(fixture.0.clone());
    let prompt = resolve("在光标生成32个别名9999");
    let started = Instant::now();
    assert_eq!(
        lib.resolve(&prompt).unwrap().definitions[0].id,
        "pattern-09999"
    );
    let cold = started.elapsed();
    let started = Instant::now();
    for _ in 0..100 {
        let result = lib.resolve(&prompt).unwrap();
        assert_eq!(result.definitions.len(), 1);
        assert_eq!(result.definitions[0].text, "中文测试");
    }
    eprintln!(
        "10k resolve: cold {cold:?}; warm average {:?}",
        started.elapsed() / 100
    );
}

#[test]
fn user_terms_aliases_completeness_and_sources_are_preserved() {
    let mut lib = bundled();
    let page = lib.search(&search("交互")).unwrap();
    assert_eq!(page.hits[0].id, "interaction");
    let mut r = read(&page.revision);
    r.max_bytes = 4096;
    assert!(lib.read(&r).unwrap().text.contains("左右手交替"));
    for (query, id) in [
        ("方向盘交互", "reverse-staircase"),
        ("单双", "double-single"),
        ("混FR", "mixed-rhythm"),
        ("光污染", "flash"),
    ] {
        assert_eq!(lib.search(&search(query)).unwrap().hits[0].id, id);
    }
    let page = lib.search(&search("大宇宙")).unwrap();
    assert_eq!(page.total, 1);
    r.id = "universe".into();
    let text = lib.read(&r).unwrap().text;
    assert!(text.contains("原稿描述 1") && text.contains("原稿描述 2"));
    assert_eq!(
        lib.search(&search("连续的双压")).unwrap().hits[0].completeness,
        "name_only"
    );
    assert!(lib.search(&search("TAP 交互")).unwrap().total > 1);
}

#[test]
fn utf8_slices_and_long_simai_are_lazy_and_versioned() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("body.md"), "中文测试").unwrap();
    // Sparse 1 GiB reference: no query or read should allocate its full size.
    let mut sample = File::create(fixture.0.join("sample.simai")).unwrap();
    use std::io::Write;
    sample.write_all(b"(120){8}3,6,3,6,").unwrap();
    sample.set_len(1024 * 1024 * 1024).unwrap();
    fixture.index("v1", vec![entry("interaction")]);
    let mut lib = Library::new(fixture.0.clone());
    let mut r = read("v1");
    let mut text = String::new();
    loop {
        let page = lib.read(&r).unwrap();
        assert!(page.text.len() <= 4);
        text.push_str(&page.text);
        let Some(offset) = page.next_offset else {
            break;
        };
        r.offset = offset;
        r.file_revision = Some(page.file_revision);
    }
    assert_eq!(text, "中文测试");
    r = read("v1");
    r.part = Some("sample".into());
    r.max_bytes = 16;
    let page = lib.read(&r).unwrap();
    assert_eq!(page.text, "(120){8}3,6,3,6,");
    assert_eq!(page.total_bytes, 1024 * 1024 * 1024);
    r.offset = page.next_offset.unwrap();
    r.file_revision = Some(page.file_revision);
    sample.set_len(1024 * 1024 * 1024 - 1).unwrap();
    assert_eq!(lib.read(&r).unwrap_err().code, ErrorCode::DocumentChanged);
    r = read("v1");
    let first = lib.read(&r).unwrap();
    r.offset = first.next_offset.unwrap();
    r.file_revision = Some(first.file_revision);
    let body_path = fixture.0.join("body.md");
    let modified = fs::metadata(&body_path).unwrap().modified().unwrap();
    // Same byte length still invalidates continuation, independently of size.
    fs::write(&body_path, "甲乙丙丁").unwrap();
    File::options()
        .write(true)
        .open(&body_path)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(modified + Duration::from_secs(1)))
        .unwrap();
    assert_eq!(lib.read(&r).unwrap_err().code, ErrorCode::DocumentChanged);
    r = read("v1");
    let fresh = lib.read(&r).unwrap();
    assert_eq!(fresh.text, "甲");
    r.offset = 1;
    r.file_revision = Some(fresh.file_revision);
    assert!(
        lib.read(&r)
            .unwrap_err()
            .message
            .contains("codepoint boundary")
    );
}

#[test]
fn revision_reload_strict_arguments_and_path_confinement() {
    let fixture = Fixture::new();
    fixture.index("v1", vec![entry("interaction")]);
    let mut lib = Library::new(fixture.0.clone());
    let first = lib.search(&search("")).unwrap();
    fixture.index("version-two", vec![entry("interaction"), entry("second")]);
    let mut s = search("");
    s.revision = Some(first.revision);
    assert_eq!(lib.search(&s).unwrap_err().code, ErrorCode::DocumentChanged);
    s.revision = None;
    assert_eq!(lib.search(&s).unwrap().total, 2);
    let mut malicious = entry("interaction");
    malicious["body"]["path"] = json!("../outside");
    fixture.index("unsafe", vec![malicious]);
    assert!(lib.read(&read("unsafe")).is_err());
    for args in [
        json!({"schema_version":1,"query":"交互","limit":21}),
        json!({"schema_version":1,"query":"交互","limit":5,"offset":1}),
        json!({"schema_version":1,"query":"交互","limit":5,"invented":true}),
    ] {
        assert!(catalog::parse("library_search", args).is_err());
    }
    for (key, value) in [
        ("id", json!("../secret")),
        ("max_bytes", json!(8193)),
        ("offset", json!(3)),
    ] {
        let mut args = serde_json::to_value(read("unsafe")).unwrap();
        args[key] = value;
        assert!(catalog::parse("library_read", args).is_err());
    }
    #[cfg(unix)]
    {
        let outside = Fixture::new();
        fs::write(outside.0.join("secret"), "中文测试").unwrap();
        std::os::unix::fs::symlink(outside.0.join("secret"), fixture.0.join("body.md")).unwrap();
        fixture.index("symlink", vec![entry("interaction")]);
        assert!(lib.read(&read("symlink")).is_err());
    }
}

#[test]
fn ten_thousand_entries_search_without_any_bodies_or_reference_files() {
    let fixture = Fixture::new();
    let entries = (0..10_000)
        .map(|i| {
            let mut e = entry(&format!("pattern-{i:05}"));
            e["name"] = json!(format!("配置{i}"));
            e["aliases"] = json!([format!("别名{i}")]);
            e
        })
        .collect();
    fixture.index("large", entries);
    let mut lib = Library::new(fixture.0.clone());
    let started = Instant::now();
    assert_eq!(
        lib.search(&search("别名9999")).unwrap().hits[0].id,
        "pattern-09999"
    );
    let cold = started.elapsed();
    let started = Instant::now();
    for _ in 0..100 {
        assert_eq!(lib.search(&search("别名9999")).unwrap().total, 1);
    }
    eprintln!(
        "10k library: index {} bytes; cold {:?}; warm average {:?}",
        fs::metadata(fixture.0.join("catalog.json")).unwrap().len(),
        cold,
        started.elapsed() / 100
    );
    let mut s = search("");
    s.limit = 20;
    let mut seen = HashSet::new();
    loop {
        let page = lib.search(&s).unwrap();
        for hit in page.hits {
            assert!(seen.insert(hit.id));
        }
        let Some(next) = page.next_offset else { break };
        s.offset = next;
        s.revision = Some(page.revision);
    }
    assert_eq!(seen.len(), 10_000);
    assert_eq!(
        fs::read_dir(&fixture.0).unwrap().count(),
        1,
        "Only metadata exists; search cannot read bodies or Simai"
    );
}

#[test]
fn bridge_reads_library_without_pumping_editor_and_enforces_permissions() {
    let fixture = Fixture::new();
    fixture.index("worker", vec![entry("interaction")]);
    let (client, _inbox) = bridge::channel_with_library(fixture.0.clone());
    let args = json!({"schema_version":1,"query":"定轨交互","limit":3});
    assert_eq!(
        client
            .call(
                "library_search",
                args.clone(),
                Permissions::default(),
                Duration::from_secs(2)
            )
            .err()
            .unwrap()
            .code,
        ErrorCode::PermissionDenied
    );
    let pending = client
        .call(
            "library_search",
            args,
            Permissions::READ_ONLY,
            Duration::from_secs(2),
        )
        .unwrap();
    loop {
        if let Some(result) = pending.poll() {
            assert_eq!(result.unwrap()["hits"][0]["id"], "interaction");
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    client.session().cancel();
    assert_eq!(
        client
            .call(
                "library_search",
                json!({"schema_version":1,"query":"","limit":3}),
                Permissions::READ_ONLY,
                Duration::from_secs(2)
            )
            .err()
            .unwrap()
            .code,
        ErrorCode::Cancelled
    );
}
