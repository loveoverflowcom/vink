//! Generates the deterministic #234 journey corpus (200 works / 1,000 comments) reusing
//! #229's corpus generator and real `vink_app::domain` validators/serializers.
#[allow(dead_code)]
#[path = "../tests/support/document_budget.rs"]
mod corpus;
#[path = "../tests/support/perf_journey.rs"]
mod perf_journey;

fn main() {
    let mut out: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--out" {
            out = args.next();
        } else {
            eprintln!("unknown argument: {arg}");
            std::process::exit(2);
        }
    }
    let manifest = perf_journey::generate();
    let json = serde_json::to_vec(&manifest).expect("serialize perf journey manifest");
    if let Some(path) = out {
        if let Some(parent) = std::path::Path::new(&path).parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).expect("create output directory");
        }
        std::fs::write(&path, &json).expect("write perf journey manifest");
        println!(
            "perf_journey_corpus: works={} comments={} digest={} -> {path}",
            manifest.work_count, manifest.comment_count, manifest.corpus_digest
        );
    } else {
        use std::io::Write;
        std::io::stdout().write_all(&json).expect("write stdout");
    }
}
