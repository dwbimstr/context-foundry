use usearch::{Index, IndexOptions, MetricKind, ScalarKind};
fn main() -> anyhow::Result<()> {
    let options = IndexOptions {
        dimensions: 2048,
        metric: MetricKind::Cos,
        quantization: ScalarKind::F32,
        ..Default::default()
    };
    let index = Index::new(&options)?;
    index.reserve(3)?;
    let mut a = vec![0f32; 2048];
    a[0] = 1.;
    let mut b = a.clone();
    b[0] = 0.;
    b[1] = 1.;
    index.add(11, &a)?;
    index.add(22, &b)?;
    anyhow::ensure!(index.search(&a, 1)?.keys == vec![11]);
    let path = std::env::args().nth(1).expect("owned scratch index path");
    index.save(&path)?;
    let reopened = Index::new(&options)?;
    reopened.load(&path)?;
    anyhow::ensure!(reopened.search(&a, 1)?.keys == vec![11]);
    anyhow::ensure!(reopened.remove(11)? == 1);
    reopened.add(11, &b)?;
    let changed = reopened.search(&a, 2)?;
    anyhow::ensure!(changed.distances.iter().all(|v| *v > 0.9));
    reopened.save(&path)?;
    let final_index = Index::new(&options)?;
    final_index.load(&path)?;
    anyhow::ensure!(final_index.size() == 2);
    println!(
        "{}",
        serde_json::json!({"usearch":"2.26.2","dimensions":2048,"add_search_save_load_remove_readd":true,"scope":"two synthetic vectors; no scale or crash proof"})
    );
    Ok(())
}
