use super::*;

pub(super) fn cmd_query(root: &Path, term: &str, json_mode: bool) -> Result<()> {
    let db = ensure_initialized(root)?;

    if json_mode {
        let results = db.search(term)?;
        let entries: Vec<_> = results
            .iter()
            .map(|(name, path, kind, signature)| {
                json!({
                    "name": name,
                    "kind": kind,
                    "signature": signature,
                    "file": path,
                })
            })
            .collect();
        println!(
            "{}",
            json!({
                "command": "query",
                "term": term,
                "count": entries.len(),
                "results": entries,
            })
        );
    } else {
        println!();
        ctx::query::execute_search(&db, term)?;
        println!();
    }

    Ok(())
}
